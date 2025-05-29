use crate::{migration::MigrationManager, CommandOutput};
use anyhow::Result;
use clap::Args;
use colored::*;

#[derive(Args)]
pub struct DownCommand {
    /// Number of migrations to rollback (default: 1)
    #[arg(short, long, default_value = "1")]
    count: usize,

    /// Dry run mode - show what would be rolled back without executing
    #[arg(long)]
    dry_run: bool,

    /// Force rollback even if DOWN section is missing (dangerous)
    #[arg(long)]
    force: bool,
}

impl DownCommand {
    pub async fn execute(&self, manager: &mut MigrationManager) -> Result<CommandOutput> {
        let applied_migrations = manager.get_applied_migrations().await?;

        if applied_migrations.is_empty() {
            return Ok(CommandOutput::success(format!(
                "{} No applied migrations to rollback",
                "✅".green()
            )));
        }

        // Get the most recent migrations to rollback (reverse order)
        let mut migrations_to_rollback: Vec<_> = applied_migrations
            .into_iter()
            .rev()
            .take(self.count)
            .collect();

        if self.dry_run {
            return self.show_dry_run(&migrations_to_rollback);
        }

        let mut rollback_count = 0;
        let mut rolled_back_migrations = Vec::new();

        for migration_record in &migrations_to_rollback {
            match manager.rollback_migration(&migration_record.version).await {
                Ok(_) => {
                    rollback_count += 1;
                    rolled_back_migrations.push(&migration_record.version);
                    println!(
                        "{} Rolled back migration: {}",
                        "✅".green(),
                        migration_record.version.bright_cyan()
                    );
                }
                Err(crate::MigrationError::RollbackError { version, reason }) => {
                    if self.force {
                        // Force rollback by just removing the record
                        match manager.remove_migration_record(&version).await {
                            Ok(_) => {
                                rollback_count += 1;
                                rolled_back_migrations.push(&migration_record.version);
                                println!(
                                    "{} Force rolled back migration: {} ({})",
                                    "⚠️ ".yellow(),
                                    version.bright_cyan(),
                                    reason.dimmed()
                                );
                            }
                            Err(e) => {
                                let error_msg = format!(
                                    "Failed to force rollback migration {}: {}",
                                    version, e
                                );

                                return Ok(CommandOutput::success_with_data(
                                    format!(
                                        "{} Rolled back {} migration(s), failed on: {}",
                                        if rollback_count > 0 { "⚠️ " } else { "❌" },
                                        rollback_count,
                                        version
                                    ),
                                    serde_json::json!({
                                        "rollback_count": rollback_count,
                                        "rolled_back_migrations": rolled_back_migrations,
                                        "failed_migration": version,
                                        "error": error_msg
                                    })
                                ));
                            }
                        }
                    } else {
                        let error_msg = format!(
                            "Cannot rollback migration {}: {}. Use --force to remove the migration record anyway.",
                            version, reason
                        );

                        return Ok(CommandOutput::success_with_data(
                            format!(
                                "{} Rolled back {} migration(s), failed on: {}",
                                if rollback_count > 0 { "⚠️ " } else { "❌" },
                                rollback_count,
                                version
                            ),
                            serde_json::json!({
                                "rollback_count": rollback_count,
                                "rolled_back_migrations": rolled_back_migrations,
                                "failed_migration": version,
                                "error": error_msg
                            })
                        ));
                    }
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to rollback migration {}: {}",
                        migration_record.version, e
                    );

                    return Ok(CommandOutput::success_with_data(
                        format!(
                            "{} Rolled back {} migration(s), failed on: {}",
                            if rollback_count > 0 { "⚠️ " } else { "❌" },
                            rollback_count,
                            migration_record.version
                        ),
                        serde_json::json!({
                            "rollback_count": rollback_count,
                            "rolled_back_migrations": rolled_back_migrations,
                            "failed_migration": migration_record.version,
                            "error": error_msg
                        })
                    ));
                }
            }
        }

        let message = if rollback_count == 1 {
            format!("{} Rolled back 1 migration successfully", "🎉".green())
        } else {
            format!("{} Rolled back {} migrations successfully", "🎉".green(), rollback_count)
        };

        Ok(CommandOutput::success_with_data(
            message,
            serde_json::json!({
                "rollback_count": rollback_count,
                "rolled_back_migrations": rolled_back_migrations
            })
        ))
    }

    fn show_dry_run(&self, migrations: &[crate::MigrationRecord]) -> Result<CommandOutput> {
        let mut output = vec![
            format!("{} Dry run mode - showing migrations that would be rolled back:", "🔍".cyan()),
            String::new(),
        ];

        for (i, migration) in migrations.iter().enumerate() {
            output.push(format!(
                "{}. {} - {} (applied at: {})",
                i + 1,
                migration.version.bright_cyan(),
                migration.description,
                crate::utils::format_timestamp(migration.applied_at).dimmed()
            ));
        }

        if migrations.is_empty() {
            output.push("No migrations would be rolled back.".to_string());
        } else {
            output.push(String::new());
            output.push(format!(
                "Total: {} migration(s) would be rolled back",
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
                        "description": m.description,
                        "applied_at": m.applied_at
                    })
                }).collect::<Vec<_>>()
            })
        ))
    }
}
