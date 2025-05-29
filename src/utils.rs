use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs;

/// Generate a timestamp-based migration version
pub fn generate_migration_version() -> String {
    let now = Utc::now();
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// Calculate SHA256 checksum of a string
pub fn calculate_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Calculate SHA256 checksum of a file
pub async fn calculate_file_checksum<P: AsRef<Path>>(file_path: P) -> Result<String, std::io::Error> {
    let content = fs::read_to_string(file_path).await?;
    Ok(calculate_checksum(&content))
}

/// Extract description from migration filename
pub fn extract_description_from_filename(filename: &str) -> String {
    // Expected format: 20250115_001_add_user_table.cql
    let stem = filename.trim_end_matches(".cql");

    // Split by underscore and take everything after the second underscore
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() >= 3 {
        parts[2..].join("_").replace('_', " ")
    } else {
        stem.to_string()
    }
}

/// Extract version from migration filename
pub fn extract_version_from_filename(filename: &str) -> Option<String> {
    // Expected format: 20250115_001_add_user_table.cql
    let stem = filename.trim_end_matches(".cql");

    // Check if it matches the expected pattern: YYYYMMDD_NNN_description
    if let Some(version_part) = stem.split('_').take(2).collect::<Vec<_>>().join("_").into() {
        // Validate that the first part is a valid date format
        if version_part.len() >= 9 {
            let (date_part, seq_part) = version_part.split_at(8);
            if date_part.chars().all(|c| c.is_ascii_digit()) &&
                seq_part.starts_with('_') &&
                seq_part[1..].chars().all(|c| c.is_ascii_digit()) {
                return Some(stem.to_string());
            }
        }
    }

    None
}

/// Format a timestamp for display
pub fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Validate migration filename format
pub fn is_valid_migration_filename(filename: &str) -> bool {
    extract_version_from_filename(filename).is_some()
}

/// Create a normalized migration filename
pub fn create_migration_filename(description: &str) -> String {
    let version = generate_migration_version();
    let normalized_desc = description
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase();

    format!("{}_{}.cql", version, normalized_desc)
}

/// Parse migration content to extract UP and DOWN sections
pub fn parse_migration_content(content: &str) -> Result<(String, Option<String>), String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut up_section = Vec::new();
    let mut down_section = Vec::new();
    let mut current_section = None;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("-- UP") || trimmed.starts_with("-- +migrate Up") {
            current_section = Some("UP");
            continue;
        } else if trimmed.starts_with("-- DOWN") || trimmed.starts_with("-- +migrate Down") {
            current_section = Some("DOWN");
            continue;
        }

        // Skip comments and empty lines at the beginning
        if current_section.is_none() && (trimmed.is_empty() || trimmed.starts_with("--")) {
            continue;
        }

        // If no section marker found, assume it's all UP
        if current_section.is_none() {
            current_section = Some("UP");
        }

        match current_section {
            Some("UP") => up_section.push(line),
            Some("DOWN") => down_section.push(line),
            _ => {}
        }
    }

    let up_content = up_section.join("\n").trim().to_string();
    let down_content = if down_section.is_empty() {
        None
    } else {
        Some(down_section.join("\n").trim().to_string())
    };

    if up_content.is_empty() {
        return Err("Migration must contain at least UP section with CQL statements".to_string());
    }

    Ok((up_content, down_content))
}

/// Generate migration template content
pub fn generate_migration_template(description: &str) -> String {
    format!(
        r#"-- Migration: {}
-- Created at: {}

-- +migrate Up
-- Add your UP migration statements here
-- Example:
-- CREATE TABLE IF NOT EXISTS example_table (
--     id UUID PRIMARY KEY,
--     name TEXT,
--     created_at TIMESTAMP
-- );

-- +migrate Down
-- Add your DOWN migration statements here (optional)
-- Example:
-- DROP TABLE IF EXISTS example_table;
"#,
        description,
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_description_from_filename() {
        assert_eq!(
            extract_description_from_filename("20250115_001_add_user_table.cql"),
            "add user table"
        );
        assert_eq!(
            extract_description_from_filename("20250115_002_create_indexes.cql"),
            "create indexes"
        );
    }

    #[test]
    fn test_extract_version_from_filename() {
        assert_eq!(
            extract_version_from_filename("20250115_001_add_user_table.cql"),
            Some("20250115_001_add_user_table".to_string())
        );
        assert_eq!(
            extract_version_from_filename("invalid_filename.cql"),
            None
        );
    }

    #[test]
    fn test_calculate_checksum() {
        let content = "CREATE TABLE test (id UUID PRIMARY KEY);";
        let checksum = calculate_checksum(content);
        assert_eq!(checksum.len(), 64); // SHA256 produces 64 hex characters
    }

    #[test]
    fn test_parse_migration_content() {
        let content = r#"
-- Migration description

-- +migrate Up
CREATE TABLE users (id UUID PRIMARY KEY);

-- +migrate Down
DROP TABLE users;
"#;

        let (up, down) = parse_migration_content(content).unwrap();
        assert!(up.contains("CREATE TABLE users"));
        assert!(down.unwrap().contains("DROP TABLE users"));
    }
}