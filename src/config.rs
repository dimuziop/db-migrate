use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub migrations: MigrationsConfig,
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub hosts: Vec<String>,
    pub keyspace: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_datacenter")]
    pub datacenter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationsConfig {
    #[serde(default = "default_migrations_dir")]
    pub directory: PathBuf,
    #[serde(default = "default_table_name")]
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_true")]
    pub auto_create_keyspace: bool,
    #[serde(default = "default_true")]
    pub verify_checksums: bool,
    #[serde(default = "default_false")]
    pub allow_destructive: bool,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

// Default value functions
fn default_port() -> u16 {
    9042
}

fn default_datacenter() -> String {
    "datacenter1".to_string()
}

fn default_migrations_dir() -> PathBuf {
    PathBuf::from("./migrations")
}

fn default_table_name() -> String {
    "schema_migrations".to_string()
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_timeout() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                hosts: vec!["127.0.0.1".to_string()],
                keyspace: "migrations_test".to_string(),
                username: String::new(),
                password: String::new(),
                port: default_port(),
                datacenter: default_datacenter(),
            },
            migrations: MigrationsConfig {
                directory: default_migrations_dir(),
                table_name: default_table_name(),
            },
            behavior: BehaviorConfig {
                auto_create_keyspace: default_true(),
                verify_checksums: default_true(),
                allow_destructive: default_false(),
                timeout_seconds: default_timeout(),
            },
        }
    }
}

impl Config {
    /// Load configuration from file and environment variables
    pub async fn load<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let mut config = if config_path.as_ref().exists() {
            let content = fs::read_to_string(config_path).await?;
            toml::from_str::<Config>(&content)?
        } else {
            tracing::info!("Config file not found, using defaults");
            Config::default()
        };

        // Override with environment variables if present
        config.override_from_env();

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Override configuration values from environment variables
    fn override_from_env(&mut self) {
        if let Ok(hosts) = std::env::var("DB_MIGRATE_HOSTS") {
            self.database.hosts = hosts
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }

        if let Ok(keyspace) = std::env::var("DB_MIGRATE_KEYSPACE") {
            self.database.keyspace = keyspace;
        }

        if let Ok(username) = std::env::var("DB_MIGRATE_USERNAME") {
            self.database.username = username;
        }

        if let Ok(password) = std::env::var("DB_MIGRATE_PASSWORD") {
            self.database.password = password;
        }

        if let Ok(migrations_dir) = std::env::var("DB_MIGRATE_MIGRATIONS_DIR") {
            self.migrations.directory = PathBuf::from(migrations_dir);
        }

        if let Ok(table_name) = std::env::var("DB_MIGRATE_TABLE_NAME") {
            self.migrations.table_name = table_name;
        }

        if let Ok(auto_create) = std::env::var("DB_MIGRATE_AUTO_CREATE_KEYSPACE") {
            self.behavior.auto_create_keyspace = auto_create.parse().unwrap_or(true);
        }

        if let Ok(verify_checksums) = std::env::var("DB_MIGRATE_VERIFY_CHECKSUMS") {
            self.behavior.verify_checksums = verify_checksums.parse().unwrap_or(true);
        }

        if let Ok(allow_destructive) = std::env::var("DB_MIGRATE_ALLOW_DESTRUCTIVE") {
            self.behavior.allow_destructive = allow_destructive.parse().unwrap_or(false);
        }
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        if self.database.hosts.is_empty() {
            anyhow::bail!("At least one database host must be specified");
        }

        if self.database.keyspace.is_empty() {
            anyhow::bail!("Database keyspace must be specified");
        }

        if self.migrations.table_name.is_empty() {
            anyhow::bail!("Migrations table name cannot be empty");
        }

        // Validate that migrations directory exists or can be created
        if !self.migrations.directory.exists() {
            if let Some(parent) = self.migrations.directory.parent() {
                if !parent.exists() {
                    anyhow::bail!(
                        "Migrations directory parent '{}' does not exist",
                        parent.display()
                    );
                }
            }
        }

        Ok(())
    }

    /// Get the full connection string for ScyllaDB
    pub fn connection_uri(&self) -> String {
        format!(
            "{}:{}",
            self.database.hosts.join(","),
            self.database.port
        )
    }

    /// Create a default configuration file
    pub async fn create_default_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let config = Config::default();
        let content = toml::to_string_pretty(&config)?;
        fs::write(path, content).await?;
        Ok(())
    }
}