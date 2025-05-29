
use crate::{migration::MigrationManager, CommandOutput, MigrationError};
use anyhow::Result;
use clap::Args;
use colored::*;

#[derive(Args)]
pub struct VerifyCommand {
    /// Fix checksum mismatches automatically (dangerous)
    #[arg(long)]
    fix: bool,
}

impl VerifyCommand {
    pub async fn execute(&self, manager: &MigrationManager) -> Result<CommandOutput> {
        let errors = manager.verify_migrations().await?;

        if errors.is_empty() {
            return Ok(CommandOutput::success(format!(
                "{} All migrations verified successfully - no integrity issues found",
                "✅".green()
            )));
        }

        let mut output = Vec::new();
        output.push(format!("{} Migration integrity issues found:", "⚠️ ".yellow()));
        output.push(String::new());

        let mut checksum_errors = Vec::new();
        let mut missing_errors = Vec::new();

        for error in &errors {
            match error {
                MigrationError::ChecksumMismatch { version, expected, actual } => {
                    checksum_errors.push((version, expected, actual));
                    output.push(format!(
                        "  {} Checksum mismatch for migration: {}",
                        "❌".red(),
                        version.bright_cyan()
                    ));
                    output.push(format!(
                        "     Expected: {}",
                        expected.dimmed()
                    ));
                    output.push(format!(
                        "     Actual:   {}",
                        actual.dimmed()
                    ));
                    output.push(String::new());
                }
                MigrationError::MigrationNotFound(version) => {
                    missing_errors.push(version);
                    output.push(format!(
                        "  {} Migration file missing: {}",
                        "❌".red(),
                        version.bright_cyan()
                    ));
                    output.push(String::new());
                }
                _ => {
                    output.push(format!(
                        "  {} Other error: {}",
                        "❌".red(),
                        error.to_string()
                    ));
                    output.push(String::new());
                }
            }
        }

        // Summary
        output.push("Summary:".bold().to_string());
        if !checksum_errors.is_empty() {
            output.push(format!(
                "  • {} migration(s) with checksum mismatches",
                checksum_errors.len()
            ));
        }
        if !missing_errors.is_empty() {
            output.push(format!(
                "  • {} migration(s) with missing files",
                missing_errors.len()
            ));
        }

        output.push(String::new());

        if self.fix && !checksum_errors.is_empty() {
            output.push(format!("{} Attempting to fix checksum mismatches...", "🔧".cyan()));

            let mut fixed_count = 0;
            for (version, _expected, actual) in &checksum_errors {
                match self.fix_checksum_mismatch(manager, version, actual).await {
                    Ok(_) => {
                        fixed_count += 1;
                        output.push(format!(
                            "  {} Fixed checksum for: {}",
                            "✅".green(),
                            version.bright_cyan()
                        ));
                    }
                    Err(e) => {
                        output.push(format!(
                            "  {} Failed to fix {}: {}",
                            "❌".red(),
                            version.bright_cyan(),
                            e.to_string().dimmed()
                        ));
                    }
                }
            }

            if fixed_count > 0 {
                output.push(String::new());
                output.push(format!(
                    "{} Fixed {} checksum mismatch(es)",
                    "✅".green(),
                    fixed_count
                ));
            }
        } else if !checksum_errors.is_empty() {
            output.push(format!(
                "{} Use --fix to automatically update checksums in the database",
                "💡".bright_blue()
            ));
        }

        if !missing_errors.is_empty() {
            output.push(format!(
                "{} Missing migration files cannot be automatically fixed",
                "⚠️ ".yellow()
            ));
            output.push("   These migrations were applied but their files are missing.".dimmed().to_string());
            output.push("   You may need to recreate them or remove the records manually.".dimmed().to_string());
        }

        Ok(CommandOutput::success_with_data(
            output.join("\n"),
            serde_json::json!({
                "integrity_issues": errors.len(),
                "checksum_mismatches": checksum_errors.len(),
                "missing_files": missing_errors.len(),
                "fixed": self.fix,
                "issues": errors.iter().map(|e| {
                    match e {
                        MigrationError::ChecksumMismatch { version, expected, actual } => {
                            serde_json::json!({
                                "type": "checksum_mismatch",
                                "version": version,
                                "expected_checksum": expected,
                                "actual_checksum": actual
                            })
                        }
                        MigrationError::MigrationNotFound(version) => {
                            serde_json::json!({
                                "type": "missing_file",
                                "version": version
                            })
                        }
                        _ => {
                            serde_json::json!({
                                "type": "other",
                                "error": e.to_string()
                            })
                        }
                    }
                }).collect::<Vec<_>>()
            })
        ))
    }

    async fn fix_checksum_mismatch(
        &self,
        manager: &MigrationManager,
        version: &str,
        new_checksum: &str,
    ) -> Result<()> {
        // We'll need to add this method to MigrationManager
        manager.update_migration_checksum(version, new_checksum).await?;
        Ok(())
    }
}