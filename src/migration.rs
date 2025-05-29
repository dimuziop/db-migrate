use crate::{
    config::Config,
    utils::{calculate_checksum, extract_version_from_filename, parse_migration_content},
    MigrationError, MigrationFile, MigrationRecord,
};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use scylla::{Session, SessionBuilder};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// Main migration manager that handles all migration operations
pub struct MigrationManager {
    session: Session,
    config: Config,
}

impl MigrationManager {
    /// Create a new migration manager and establish database connection
    pub async fn new(config: Config) -> Result<Self, MigrationError> {
        info!("Connecting to ScyllaDB at: {:?}", config.database.hosts);

        let mut session_builder = SessionBuilder::new().known_nodes(&config.database.hosts);

        if !config.database.username.is_empty() {
            session_builder =
                session_builder.user(&config.database.username, &config.database.password);
        }

        let session = session_builder.build().await?;

        let manager = Self { session, config };

        // Ensure keyspace and migrations table exist
        manager.initialize_schema().await?;

        Ok(manager)
    }

    /// Initialize the keyspace and migrations tracking table
    async fn initialize_schema(&self) -> Result<(), MigrationError> {
        // Create keyspace if it doesn't exist and auto_create is enabled
        if self.config.behavior.auto_create_keyspace {
            let create_keyspace_query = format!(
                "CREATE KEYSPACE IF NOT EXISTS {} WITH REPLICATION = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
                self.config.database.keyspace
            );

            debug!("Creating keyspace: {}", create_keyspace_query);
            self.session.query(create_keyspace_query, &[]).await?;
        }

        // Use the keyspace
        let use_keyspace_query = format!("USE {}", self.config.database.keyspace);
        self.session.query(use_keyspace_query, &[]).await?;

        // Create migrations table
        let create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                version TEXT PRIMARY KEY,
                applied_at TIMESTAMP,
                checksum TEXT,
                description TEXT
            )",
            self.config.migrations.table_name
        );

        debug!("Creating migrations table: {}", create_table_query);
        self.session.query(create_table_query, &[]).await?;

        info!("Schema initialization completed");
        Ok(())
    }

    /// Get all applied migrations from the database
    pub async fn get_applied_migrations(&self) -> Result<Vec<MigrationRecord>, MigrationError> {
        let query = format!(
            "SELECT version, applied_at, checksum, description FROM {} ORDER BY version",
            self.config.migrations.table_name
        );

        let rows = self.session.query(query, &[]).await?;
        let mut migrations = Vec::new();

        for row in rows
            .rows_typed::<(String, i64, String, String)>()
            .map_err(|e| MigrationError::IntegrityError(e.to_string()))?
        {
            let (version, applied_at_ts, checksum, description) =
                row.map_err(|e| MigrationError::IntegrityError(e.to_string()))?;

            let applied_at = Utc
                .timestamp_millis_opt(applied_at_ts)
                .single()
                .ok_or_else(|| MigrationError::IntegrityError("Invalid timestamp".into()))?;

            migrations.push(MigrationRecord {
                version,
                applied_at,
                checksum,
                description,
            });
        }

        Ok(migrations)
    }

    /// Get all migration files from the filesystem
    pub async fn get_migration_files(&self) -> Result<Vec<MigrationFile>, MigrationError> {
        let migrations_dir = &self.config.migrations.directory;

        if !migrations_dir.exists() {
            fs::create_dir_all(migrations_dir).await?;
            return Ok(Vec::new());
        }

        let mut files = Vec::new();

        for entry in WalkDir::new(migrations_dir)
            .min_depth(1)
            .max_depth(1)
            .sort_by_file_name()
        {
            let entry = entry.map_err(|e| MigrationError::ConfigError(e.to_string()))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("cql") {
                continue;
            }

            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| MigrationError::InvalidFormat("Invalid filename".to_string()))?;

            if let Some(version) = extract_version_from_filename(filename) {
                let content = fs::read_to_string(path).await?;
                let checksum = calculate_checksum(&content);
                let description = crate::utils::extract_description_from_filename(filename);

                files.push(MigrationFile {
                    version,
                    description,
                    file_path: path.to_path_buf(),
                    content,
                    checksum,
                });
            } else {
                warn!("Skipping file with invalid format: {}", filename);
            }
        }

        Ok(files)
    }

    /// Get pending migrations (files that haven't been applied)
    pub async fn get_pending_migrations(&self) -> Result<Vec<MigrationFile>, MigrationError> {
        let applied = self.get_applied_migrations().await?;
        let files = self.get_migration_files().await?;

        let applied_versions: std::collections::HashSet<String> =
            applied.into_iter().map(|m| m.version).collect();

        let pending: Vec<MigrationFile> = files
            .into_iter()
            .filter(|f| !applied_versions.contains(&f.version))
            .collect();

        Ok(pending)
    }

    /// Apply a single migration
    pub async fn apply_migration(
        &mut self,
        migration: &MigrationFile,
    ) -> Result<(), MigrationError> {
        info!("Applying migration: {}", migration.version);

        // Check if already applied
        if self.is_migration_applied(&migration.version).await? {
            return Err(MigrationError::AlreadyApplied {
                version: migration.version.clone(),
            });
        }

        // Parse migration content
        let (up_content, _down_content) = parse_migration_content(&migration.content)
            .map_err(|e| MigrationError::InvalidFormat(e))?;

        // Execute UP statements
        for statement in split_cql_statements(&up_content) {
            if !statement.trim().is_empty() {
                debug!("Executing: {}", statement.trim());
                self.session.query(statement, &[]).await?;
            }
        }

        // Record the migration as applied
        self.record_migration_applied(migration).await?;

        info!("✅ Applied migration: {}", migration.version);
        Ok(())
    }

    /// Rollback a single migration
    pub async fn rollback_migration(&mut self, version: &str) -> Result<(), MigrationError> {
        info!("Rolling back migration: {}", version);

        // Check if migration is applied
        if !self.is_migration_applied(version).await? {
            return Err(MigrationError::MigrationNotFound(version.to_string()));
        }

        // Find the migration file
        let files = self.get_migration_files().await?;
        let migration_file = files
            .iter()
            .find(|f| f.version == version)
            .ok_or_else(|| MigrationError::MigrationNotFound(version.to_string()))?;

        // Parse migration content
        let (_up_content, down_content) = parse_migration_content(&migration_file.content)
            .map_err(|e| MigrationError::InvalidFormat(e))?;

        let down_content = down_content.ok_or_else(|| MigrationError::RollbackError {
            version: version.to_string(),
            reason: "No DOWN section found in migration".to_string(),
        })?;

        // Execute DOWN statements
        for statement in split_cql_statements(&down_content) {
            if !statement.trim().is_empty() {
                debug!("Executing rollback: {}", statement.trim());
                self.session.query(statement, &[]).await?;
            }
        }

        // Remove the migration record
        self.remove_migration_record(version).await?;

        info!("✅ Rolled back migration: {}", version);
        Ok(())
    }

    /// Check if a migration is already applied
    pub async fn is_migration_applied(&self, version: &str) -> Result<bool, MigrationError> {
        let query = format!(
            "SELECT version FROM {} WHERE version = ? LIMIT 1",
            self.config.migrations.table_name
        );

        let rows = self.session.query(query, (version,)).await?;
        Ok(!rows.rows.unwrap_or_default().is_empty())
    }

    /// Record a migration as applied
    async fn record_migration_applied(
        &self,
        migration: &MigrationFile,
    ) -> Result<(), MigrationError> {
        let query = format!(
            "INSERT INTO {} (version, applied_at, checksum, description) VALUES (?, ?, ?, ?)",
            self.config.migrations.table_name
        );

        self.session
            .query(
                query,
                (
                    &migration.version,
                    Utc::now().timestamp_millis(),
                    &migration.checksum,
                    &migration.description,
                ),
            )
            .await?;

        Ok(())
    }

    /// Remove a migration record
    pub(crate) async fn remove_migration_record(
        &self,
        version: &str,
    ) -> Result<(), MigrationError> {
        let query = format!(
            "DELETE FROM {} WHERE version = ?",
            self.config.migrations.table_name
        );

        self.session.query(query, (version,)).await?;
        Ok(())
    }

    /// Verify migration integrity (check checksums)
    pub async fn verify_migrations(&self) -> Result<Vec<MigrationError>, MigrationError> {
        let applied = self.get_applied_migrations().await?;
        let files = self.get_migration_files().await?;

        let file_map: HashMap<String, &MigrationFile> =
            files.iter().map(|f| (f.version.clone(), f)).collect();

        let mut errors = Vec::new();

        for applied_migration in applied {
            if let Some(file) = file_map.get(&applied_migration.version) {
                if file.checksum != applied_migration.checksum {
                    errors.push(MigrationError::ChecksumMismatch {
                        version: applied_migration.version,
                        expected: applied_migration.checksum,
                        actual: file.checksum.clone(),
                    });
                }
            } else {
                errors.push(MigrationError::MigrationNotFound(applied_migration.version));
            }
        }

        Ok(errors)
    }

    /// Reset all migrations (destructive operation)
    pub async fn reset_migrations(&mut self) -> Result<(), MigrationError> {
        if !self.config.behavior.allow_destructive {
            return Err(MigrationError::ConfigError(
                "Destructive operations are disabled in configuration".to_string(),
            ));
        }

        warn!("Resetting all migrations - this is destructive!");

        // Drop and recreate the migrations table
        let drop_query = format!("DROP TABLE IF EXISTS {}", self.config.migrations.table_name);
        self.session.query(drop_query, &[]).await?;

        self.initialize_schema().await?;

        info!("✅ All migrations reset");
        Ok(())
    }

    /// Get the configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Update the checksum of an existing migration record
    pub async fn update_migration_checksum(
        &self,
        version: &str,
        new_checksum: &str,
    ) -> Result<(), MigrationError> {
        let query = format!(
            "UPDATE {} SET checksum = ? WHERE version = ?",
            self.config.migrations.table_name
        );

        self.session.query(query, (new_checksum, version)).await?;
        Ok(())
    }

    /// Create a new migration file
    pub async fn create_migration_file(
        &self,
        description: &str,
    ) -> Result<PathBuf, MigrationError> {
        let filename = crate::utils::create_migration_filename(description);
        let file_path = self.config.migrations.directory.join(&filename);

        // Ensure migrations directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Generate template content
        let content = crate::utils::generate_migration_template(description);

        // Write the file
        fs::write(&file_path, content).await?;

        info!("✅ Created migration file: {}", filename);
        Ok(file_path)
    }
}

/// Split CQL content into individual statements
fn split_cql_statements(content: &str) -> Vec<String> {
    content
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
