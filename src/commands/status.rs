use crate::{migration::MigrationManager, CommandOutput, utils::format_timestamp};
use anyhow::Result;
use clap::Args;
use colored::*;
use std::collections::HashSet;

#[derive(Args)]
pub struct StatusCommand {
    /// Show detailed information about each migration
    #[arg(short, long)]
    verbose: bool,
}

impl StatusCommand {
    pub async fn execute(&self, manager: &MigrationManager) -> Result<CommandOutput> {
        let applied_migrations = manager.get_applied_migrations().await?;
        let all_files = manager.get_migration_files().await?;
        let pending_migrations = manager.get_pending_migrations().await?;

        let applied_versions: HashSet<String> =
            applied_migrations.iter().map(|m| m.version.clone()).collect();

        let mut output = Vec::new();

        // Header
        output.push(format!("{} Migration Status", "📊".cyan()));
        output.push("═".repeat(50));
        output.push(String::new());

        // Current state summary
        let current_version = applied_migrations
            .last()
            .map(|m| m.version.as_str())
            .unwrap_or("None");

        output.push(format!(
            "{}: {}",
            "Current schema version".bold(),
            if current_version == "None" {
                "None (no migrations applied)".dimmed().to_string()
            } else {
                current_version.bright_cyan().to_string()
            }
        ));

        output.push(format!(
            "{}: {}",
            "Applied migrations".bold(),
            if applied_migrations.is_empty() {
                "0".dimmed().to_string()
            } else {
                applied_migrations.len().to_string().bright_green().to_string()
            }
        ));

        output.push(format!(
            "{}: {}",
            "Pending migrations".bold(),
            if pending_migrations.is_empty() {
                "0 ✅".bright_green().to_string()
            } else {
                format!("{} ⚠️", pending_migrations.len()).bright_yellow().to_string()
            }
        ));

        output.push(format!(
            "{}: {}",
            "Total migration files".bold(),
            all_files.len().to_string().bright_blue()
        ));

        if self.verbose {
            output.push(String::new());
            output.push("Applied Migrations:".bold().to_string());
            output.push("─".repeat(30));

            if applied_migrations.is_empty() {
                output.push("  No migrations applied yet".dimmed().to_string());
            } else {
                for migration in &applied_migrations {
                    output.push(format!(
                        "  {} {} - {} {}",
                        "✅".green(),
                        migration.version.bright_cyan(),
                        migration.description,
                        format!("({})", format_timestamp(migration.applied_at)).dimmed()
                    ));
                }
            }

            output.push(String::new());
            output.push("Pending Migrations:".bold().to_string());
            output.push("─".repeat(30));

            if pending_migrations.is_empty() {
                output.push("  No pending migrations".dimmed().to_string());
            } else {
                for migration in &pending_migrations {
                    output.push(format!(
                        "  {} {} - {}",
                        "⏳".yellow(),
                        migration.version.bright_cyan(),
                        migration.description
                    ));
                }
            }

            // Show files without valid migration format
            let invalid_files: Vec<_> = all_files
                .iter()
                .filter(|f| !applied_versions.contains(&f.version) &&
                    !pending_migrations.iter().any(|p| p.version == f.version))
                .collect();

            if !invalid_files.is_empty() {
                output.push(String::new());
                output.push("Invalid Migration Files:".bold().to_string());
                output.push("─".repeat(30));

                for file in invalid_files {
                    output.push(format!(
                        "  {} {} - {}",
                        "❌".red(),
                        file.file_path.file_name().unwrap_or_default().to_string_lossy(),
                        "Invalid format or duplicate version".red()
                    ));
                }
            }
        }

        // Status summary
        output.push(String::new());
        let status_message = if pending_migrations.is_empty() {
            format!("{} Schema is up to date", "✅".green())
        } else {
            format!(
                "{} {} migration(s) pending. Run 'db-migrate up' to apply them.",
                "⚠️ ".yellow(),
                pending_migrations.len()
            )
        };
        output.push(status_message);

        Ok(CommandOutput::success_with_data(
            output.join("\n"),
            serde_json::json!({
                "current_version": current_version,
                "applied_count": applied_migrations.len(),
                "pending_count": pending_migrations.len(),
                "total_files": all_files.len(),
                "up_to_date": pending_migrations.is_empty(),
                "applied_migrations": applied_migrations.iter().map(|m| {
                    serde_json::json!({
                        "version": m.version,
                        "description": m.description,
                        "applied_at": m.applied_at,
                        "checksum": m.checksum
                    })
                }).collect::<Vec<_>>(),
                "pending_migrations": pending_migrations.iter().map(|m| {
                    serde_json::json!({
                        "version": m.version,
                        "description": m.description,
                        "checksum": m.checksum
                    })
                }).collect::<Vec<_>>()
            })
        ))
    }
}