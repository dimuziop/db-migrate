
use crate::{migration::MigrationManager, CommandOutput};
use anyhow::Result;
use clap::Args;
use colored::*;

#[derive(Args)]
pub struct ResetCommand {
    /// Skip confirmation prompt
    #[arg(long)]
    yes: bool,
}

impl ResetCommand {
    pub async fn execute(&self, manager: &mut MigrationManager) -> Result<CommandOutput> {
        // Safety check - make sure destructive operations are allowed
        if !manager.get_config().behavior.allow_destructive {
            return Ok(CommandOutput::error(format!(
                "{} Destructive operations are disabled in configuration. Set 'allow_destructive = true' in your config file to enable reset.",
                "❌".red()
            )));
        }

        // Get current state
        let applied_migrations = manager.get_applied_migrations().await?;

        if applied_migrations.is_empty() {
            return Ok(CommandOutput::success(format!(
                "{} No migrations to reset - migration table is already empty",
                "✅".green()
            )));
        }

        // Show what will be reset
        let mut warning = vec![
            format!("{} WARNING: This will permanently delete all migration records!", "⚠️ ".bright_red().bold()),
            String::new(),
            "The following migrations will be removed from the tracking table:".to_string(),
        ];

        for migration in &applied_migrations {
            warning.push(format!(
                "  • {} - {} (applied: {})",
                migration.version.bright_cyan(),
                migration.description,
                crate::utils::format_timestamp(migration.applied_at).dimmed()
            ));
        }

        warning.push(String::new());
        warning.push(format!(
            "{} This operation will NOT drop your actual database tables or data.",
            "💡".bright_blue()
        ));
        warning.push("It only clears the migration tracking table.".dimmed().to_string());
        warning.push(String::new());
        warning.push(format!(
            "Total migrations to reset: {}",
            applied_migrations.len().to_string().bright_red().bold()
        ));

        if !self.yes {
            warning.push(String::new());
            warning.push(format!(
                "{} Use --yes to confirm this destructive operation",
                "🔒".yellow()
            ));

            return Ok(CommandOutput::success_with_data(
                warning.join("\n"),
                serde_json::json!({
                    "action": "confirmation_required",
                    "migrations_to_reset": applied_migrations.len(),
                    "destructive": true,
                    "confirmed": false
                })
            ));
        }

        // Perform the reset
        match manager.reset_migrations().await {
            Ok(_) => {
                let mut success_message = vec![
                    format!("{} Successfully reset all migrations!", "✅".green().bold()),
                    String::new(),
                    format!("• Removed {} migration record(s)", applied_migrations.len()),
                    "• Migration tracking table has been recreated".to_string(),
                    String::new(),
                    format!(
                        "{} You can now run 'db-migrate up' to reapply your migrations",
                        "💡".bright_blue()
                    ),
                ];

                Ok(CommandOutput::success_with_data(
                    success_message.join("\n"),
                    serde_json::json!({
                        "action": "reset_completed",
                        "migrations_reset": applied_migrations.len(),
                        "destructive": true,
                        "confirmed": true
                    })
                ))
            }
            Err(e) => {
                Ok(CommandOutput::error(format!(
                    "Failed to reset migrations: {}",
                    e
                )))
            }
        }
    }
}