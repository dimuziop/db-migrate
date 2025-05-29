# db-migrate - ScyllaDB Migration Tool

A robust, production-ready database migration tool for ScyllaDB/Cassandra, written in Rust.
Designed with extensibility in mind, it aims to bring a familiar approach to other databases as well. This tool offers Rails- and Django-style migrations, featuring advanced capabilities like checksum verification, rollback support, and schema drift detection.

## 🚀 Features

- **Migration Tracking**: Keeps track of applied migrations with checksums
- **Idempotent Operations**: Safe to run multiple times
- **Rollback Support**: Reverse migrations with DOWN sections
- **Checksum Verification**: Detect unauthorized changes to migration files
- **Dry Run Mode**: Preview changes before applying
- **CI/CD Ready**: JSON output and clear exit codes
- **Environment Configuration**: Config files + environment variables
- **Force Operations**: Handle edge cases safely

## 📦 Installation

### From Source

```bash
git clone <repository-url>
cd db-migrate
cargo build --release
./target/release/db-migrate --help
```

### Using Cargo

```bash
cargo install db-migrate
```

## ⚙️ Configuration

### Configuration File (`db-migrate.toml`)

```toml
[database]
hosts = ["127.0.0.1:9042"]
keyspace = "my_keyspace"
username = ""
password = ""

[migrations]
directory = "./migrations"
table_name = "schema_migrations"

[behavior]
auto_create_keyspace = true
verify_checksums = true
allow_destructive = false  # Set to true for development
```

### Environment Variables

```bash
export DB_MIGRATE_HOSTS=localhost:9042,node2:9042
export DB_MIGRATE_KEYSPACE=my_keyspace
export DB_MIGRATE_USERNAME=cassandra
export DB_MIGRATE_PASSWORD=cassandra
export DB_MIGRATE_MIGRATIONS_DIR=./migrations
export DB_MIGRATE_ALLOW_DESTRUCTIVE=false
```

## 🎯 Quick Start

### 1. Initialize Configuration

```bash
# Create default config file
./db-migrate create-config  # (if implemented)
# Or manually create db-migrate.toml with your settings
```

### 2. Create Your First Migration

```bash
./db-migrate create create_users_table
```

This creates: `migrations/20250128_143022_create_users_table.cql`

### 3. Edit the Migration File

```sql
-- Migration: create_users_table
-- Created at: 2025-01-28 14:30:22 UTC

-- +migrate Up
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT,
    name TEXT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS users_email_idx ON users (email);

-- +migrate Down
DROP INDEX IF EXISTS users_email_idx;
DROP TABLE IF EXISTS users;
```

### 4. Apply Migrations

```bash
# See what would be applied
./db-migrate up --dry-run

# Apply all pending migrations
./db-migrate up

# Apply only the next 2 migrations
./db-migrate up --count 2
```

### 5. Check Status

```bash
./db-migrate status
./db-migrate status --verbose
```

## 📋 Commands

### `create <description>`

Create a new migration file with the given description.

```bash
./db-migrate create add_user_preferences_table
./db-migrate create "alter users add column phone"
```

### `up [options]`

Apply pending migrations.

```bash
./db-migrate up                    # Apply all pending
./db-migrate up --count 3          # Apply next 3 migrations
./db-migrate up --dry-run          # Show what would be applied
```

### `down [options]`

Rollback applied migrations.

```bash
./db-migrate down                  # Rollback last migration
./db-migrate down --count 2        # Rollback last 2 migrations
./db-migrate down --dry-run        # Show what would be rolled back
./db-migrate down --force          # Force rollback even without DOWN section
```

### `status [options]`

Show current migration status.

```bash
./db-migrate status                # Basic status
./db-migrate status --verbose      # Detailed information
```

### `verify [options]`

Verify migration integrity and detect schema drift.

```bash
./db-migrate verify                # Check for issues
./db-migrate verify --fix          # Auto-fix checksum mismatches
```

### `reset [options]`

Reset all migrations (destructive).

```bash
./db-migrate reset --yes           # Reset with confirmation
```

## 📁 Migration File Format

### File Naming Convention

Files must follow the pattern: `YYYYMMDD_HHMMSS_description.cql`

Example: `20250128_143022_create_users_table.cql`

### File Structure

```sql
-- Optional: Migration description and metadata

-- +migrate Up
-- Your forward migration statements here
CREATE TABLE example (
    id UUID PRIMARY KEY,
    name TEXT
);

-- +migrate Down
-- Your rollback statements here (optional but recommended)
DROP TABLE example;
```

### Best Practices

1. **Always include DOWN sections** for reversible migrations
2. **Use IF EXISTS/IF NOT EXISTS** for idempotency
3. **One logical change per migration** (single table, index, etc.)
4. **Test rollbacks** before applying to production
5. **Descriptive names** that explain the change

## 🔧 Advanced Usage

### JSON Output for CI/CD

```bash
./db-migrate status --output json | jq '.data.pending_count'
./db-migrate up --output json
```

### Environment-Specific Configurations

```bash
# Development
./db-migrate --config dev.toml up

# Production  
./db-migrate --config prod.toml up --dry-run
```

### Handling Complex Migrations

For migrations that can't be easily reversed:

```sql
-- +migrate Up
ALTER TABLE users ADD COLUMN new_field TEXT;
-- Populate new_field with data transformation
UPDATE users SET new_field = transform(old_field);

-- +migrate Down
-- Note: This migration cannot be automatically reversed
-- Manual steps required:
-- 1. Verify no application dependencies on new_field
-- 2. Run: ALTER TABLE users DROP COLUMN new_field;
```

## 🚨 Production Considerations

### Pre-deployment Checks

```bash
# 1. Verify all migrations
./db-migrate verify

# 2. Dry run on production schema
./db-migrate up --dry-run

# 3. Check pending count
./db-migrate status --output json | jq '.data.pending_count'
```

### Safe Deployment Pattern

```bash
# 1. Backup database (external tool)
# 2. Apply migrations with monitoring
./db-migrate up --verbose

# 3. Verify application health
# 4. If issues: rollback
./db-migrate down --count N
```

### CI/CD Integration

```yaml
# Example GitHub Actions step
- name: Apply Database Migrations
  run: |
    ./db-migrate verify
    ./db-migrate up
    
    # Check exit code
    if [ $? -eq 0 ]; then
      echo "✅ Migrations applied successfully"
    else
      echo "❌ Migration failed"
      exit 1
    fi
```

## 🐛 Troubleshooting

### Common Issues

**Migration checksum mismatch:**
```bash
./db-migrate verify --fix
```

**Missing DOWN section:**
```bash
./db-migrate down --force  # Use with caution
```

**Connection issues:**
```bash
# Verify connection
./db-migrate status --verbose

# Check configuration
cat db-migrate.toml
```

**Schema drift:**
```bash
./db-migrate verify  # Identifies manual schema changes
```

### Recovery Scenarios

**Corrupted migration state:**
```bash
# Last resort - reset and reapply (DANGEROUS)
./db-migrate reset --yes
./db-migrate up
```

**Partial migration failure:**
```bash
# Check what was applied
./db-migrate status --verbose

# Manual cleanup may be needed
# Then retry: ./db-migrate up
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details.

## 🆘 Support

- File issues on GitHub for bugs/features
- Check existing issues for common problems
- Include migration files and config when reporting issues
