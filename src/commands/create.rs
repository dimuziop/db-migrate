use crate::{migration::MigrationManager, CommandOutput};
use anyhow::Result;
use clap::Args;
use colored::*;

#[derive(Args)]
pub struct CreateCommand {
    /// Description of the migration
    description: String,
}

impl CreateCommand {
    pub async fn execute(&self, manager: &MigrationManager) -> Result<CommandOutput> {
        // Validate description
        if self.description.trim().is_empty() {
            return Ok(CommandOutput::error("Migration description cannot be empty"));
        }

        // Create the migration file
        let file_path = manager.create_migration_file(&self.description).await?;

        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let message = format!(
            "{} Created migration file: {}",
            "✅".green(),
            filename.bright_cyan()
        );

        Ok(CommandOutput::success_with_data(
            message,
            serde_json::json!({
                "file_path": file_path.to_string_lossy(),
                "filename": filename
            })
        ))
    }
}