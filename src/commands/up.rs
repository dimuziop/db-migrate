use crate::{migration::MigrationManager, CommandOutput};
use anyhow::Result;
use clap::Args;
use colored::*;

#[derive(Args)]
pub struct UpCommand {
    /// Number of migrations to apply (default: all)
    #[arg(short, long)]
    count: Option<usize>,

    /// Dry run mode - show what would be applied without executing
    #[arg(long)]
    dry_run: bool,
}

impl UpCommand {
    pub async fn execute(&self, manager: &mut MigrationManager) -> Result<CommandOutput> {
        let pending_migrations = manager.get_pending_migrations().await?;

        if pending_migrations.is_empty() {
            return Ok(CommandOutput::success(format!(
                "{} No pending migrations found",
                "✅".green()
            )));
        }

        // Determine how many migrations to apply
        let migrations_to_apply = if let Some(count) = self.count {
            pending_migrations.into_iter().take(count).collect()
        } else {
            pending_migrations
        };

        if self.dry_run {
            return self.show_dry_run(&migrations_to_apply);
        }

        let mut applied_count = 0;
        let mut applied_migrations = Vec::new();

        for migration in &migrations_to_apply {
            match manager.apply_migration(migration).await {
                Ok(_) => {
                    applied_count += 1;
                    applied_migrations.push(&migration.version);
                    println!(
                        "{} Applied migration: {}",
                        "✅".green(),
                        migration.version.bright_cyan()
                    );
                }
                Err(e) => {
                    return Ok(CommandOutput::success_with_data(
                        format!(
                            "{} Applied {} migration(s), failed on: {}",
                            if applied_count > 0 { "⚠️ " } else { "❌" },
                            applied_count,
                            migration.version
                        ),
                        serde_json::json!({
                            "applied_count": applied_count,
                            "applied_migrations": applied_migrations,
                            "failed_migration": migration.version,
                            "error": e.to_string()
                        })
                    ));
                }
            }
        }

        let message = if applied_count == 1 {
            format!("{} Applied 1 migration successfully", "🎉".green())
        } else {
            format!("{} Applied {} migrations successfully", "🎉".green(), applied_count)
        };

        Ok(CommandOutput::success_with_data(
            message,
            serde_json::json!({
                "applied_count": applied_count,
                "applied_migrations": applied_migrations
            })
        ))
    }

    fn show_dry_run(&self, migrations: &[crate::MigrationFile]) -> Result<CommandOutput> {
        let mut output = vec![
            format!("{} Dry run mode - showing migrations that would be applied:", "🔍".cyan()),
            String::new(),
        ];

        for (i, migration) in migrations.iter().enumerate() {
            output.push(format!(
                "{}. {} - {}",
                i + 1,
                migration.version.bright_cyan(),
                migration.description
            ));
        }

        if migrations.is_empty() {
            output.push("No migrations would be applied.".to_string());
        } else {
            output.push(String::new());
            output.push(format!(
                "Total: {} migration(s) would be applied",
                migrations.len()
            ));
        }

        Ok(CommandOutput::success_with_data(
            output.join("\n"),
            serde_json::json!({
                "dry_run": true,
                "migrations_count": migrations.len(),
                "migrations": migrations.iter().map(|m| {
                    serde_json::json!({
                        "version": m.version,
                        "description": m.description
                    })
                }).collect::<Vec<_>>()
            })
        ))
    }
}