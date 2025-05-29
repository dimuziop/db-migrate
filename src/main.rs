use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use db_migrate::{
    config::Config,
    commands::{CreateCommand, DownCommand, StatusCommand, UpCommand, VerifyCommand, ResetCommand},
    migration::MigrationManager,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(
    name = "db-migrate",
    about = "Robust database migration tool for ScyllaDB",
    version = env!("CARGO_PKG_VERSION")
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration file path
    #[arg(short, long, default_value = "db-migrate.toml")]
    config: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    output: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new migration file
    Create(CreateCommand),
    /// Apply pending migrations
    Up(UpCommand),
    /// Rollback the last migration
    Down(DownCommand),
    /// Show current migration status
    Status(StatusCommand),
    /// Verify schema integrity
    Verify(VerifyCommand),
    /// Reset all migrations (destructive)
    Reset(ResetCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose)?;

    // Load configuration
    let config = Config::load(&cli.config).await?;

    // Create migration manager
    let mut manager = MigrationManager::new(config).await?;

    // Execute command
    let result = match cli.command {
        Commands::Create(cmd) => cmd.execute(&manager).await,
        Commands::Up(cmd) => cmd.execute(&mut manager).await,
        Commands::Down(cmd) => cmd.execute(&mut manager).await,
        Commands::Status(cmd) => cmd.execute(&manager).await,
        Commands::Verify(cmd) => cmd.execute(&manager).await,
        Commands::Reset(cmd) => cmd.execute(&mut manager).await,
    };

    match result {
        Ok(output) => {
            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("{}", output);
            }
            std::process::exit(0);
        }
        Err(e) => {
            if cli.output == "json" {
                let error_output = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });
                println!("{}", serde_json::to_string_pretty(&error_output)?);
            } else {
                eprintln!("{} {}", "Error:".red().bold(), e);

                // Show error chain
                let mut source = e.source();
                while let Some(err) = source {
                    eprintln!("  {}: {}", "Caused by".yellow(), err);
                    source = err.source();
                }
            }
            std::process::exit(1);
        }
    }
}

fn init_logging(verbose: bool) -> Result<()> {
    let filter = if verbose {
        "db_migrate=debug,info"
    } else {
        "db_migrate=info,warn,error"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}