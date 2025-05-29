pub mod commands;
pub mod config;
pub mod migration;
pub mod schema;
pub mod utils;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Represents a migration record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub version: String,
    pub applied_at: DateTime<Utc>,
    pub checksum: String,
    pub description: String,
}

/// Represents a migration file on disk
#[derive(Debug, Clone)]
pub struct MigrationFile {
    pub version: String,
    pub description: String,
    pub file_path: std::path::PathBuf,
    pub content: String,
    pub checksum: String,
}

/// Represents the result of a command execution
#[derive(Debug, Serialize)]
pub struct CommandOutput {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl CommandOutput {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

impl std::fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Custom error types for the migration tool
#[derive(thiserror::Error, Debug)]
pub enum MigrationError {
    #[error("Database connection error: {0}")]
    DatabaseError(#[from] scylla::transport::errors::NewSessionError),

    #[error("Query execution error: {0}")]
    QueryError(#[from] scylla::transport::errors::QueryError),

    #[error("Migration file error: {0}")]
    FileError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Migration integrity error: {0}")]
    IntegrityError(String),

    #[error("Migration not found: {0}")]
    MigrationNotFound(String),

    #[error("Checksum mismatch for migration {version}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        version: String,
        expected: String,
        actual: String,
    },

    #[error("Cannot rollback migration {version}: {reason}")]
    RollbackError { version: String, reason: String },

    #[error("Migration {version} is already applied")]
    AlreadyApplied { version: String },

    #[error("Invalid migration format: {0}")]
    InvalidFormat(String),
}