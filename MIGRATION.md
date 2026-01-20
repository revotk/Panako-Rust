# Fingerprint Migration Tool

`fpmigrate` is a command-line tool for migrating fingerprint data from filesystem storage to PostgreSQL.

## Features

- ✅ Migrate from filesystem (JSON/BSON) to PostgreSQL
- ✅ Dry-run mode to preview migration
- ✅ Skip existing files to avoid duplicates
- ✅ Batch processing with progress tracking
- ✅ Comprehensive error handling and logging
- ✅ Automatic metadata preservation

## Prerequisites

1. **PostgreSQL Database**: Running PostgreSQL instance (use `docker-compose up -d` in project root)
2. **Source Data**: Fingerprint files in JSON or BSON format
3. **Configuration**: PostgreSQL configuration file

## Usage

### Basic Migration

Migrate from a directory to PostgreSQL:

```bash
cargo run --bin fpmigrate -- \
  --source-dir ./fingerprints \
  --dest-config config.postgresql.toml
```

### Using Configuration Files

Migrate using source and destination configs:

```bash
cargo run --bin fpmigrate -- \
  --source-config config.toml \
  --dest-config config.postgresql.toml
```

### Dry Run

Preview what would be migrated without actually migrating:

```bash
cargo run --bin fpmigrate -- \
  --source-dir ./fingerprints \
  --dest-config config.postgresql.toml \
  --dry-run
```

### Verbose Logging

Enable detailed logging for debugging:

```bash
cargo run --bin fpmigrate -- \
  --source-dir ./fingerprints \
  --dest-config config.postgresql.toml \
  --verbose
```

### Force Re-migration

By default, existing files are skipped. To force re-migration:

```bash
cargo run --bin fpmigrate -- \
  --source-dir ./fingerprints \
  --dest-config config.postgresql.toml \
  --skip-existing=false
```

## Command-Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `--source-dir <DIR>` | Source directory with fingerprint files | - |
| `--source-config <FILE>` | Source configuration file (filesystem) | - |
| `--dest-config <FILE>` | Destination configuration file (PostgreSQL) | **Required** |
| `--dry-run` | Preview migration without executing | `false` |
| `--skip-existing` | Skip files already in destination | `true` |
| `-v, --verbose` | Enable verbose logging | `false` |

**Note**: Either `--source-dir` or `--source-config` must be provided (but not both).

## Configuration Files

### Source Configuration (config.toml)

```toml
[storage]
backend = "filesystem"

[storage.filesystem]
base_directory = "./fingerprints"
format = "auto"  # auto-detect JSON or BSON
```

### Destination Configuration (config.postgresql.toml)

```toml
[storage]
backend = "postgresql"

[storage.postgresql]
host = "localhost"
port = 5432
database = "panako"
user = "panako_user"
password = "panako_pass"
max_connections = 10
```

## Migration Process

The tool performs the following steps for each file:

1. **Load** fingerprints from source (filesystem)
2. **Check** if file already exists in destination (if `--skip-existing` is enabled)
3. **Extract** metadata from source
4. **Save** fingerprints and metadata to PostgreSQL
5. **Report** success or failure

## Output Example

```
🚀 Starting fingerprint migration
📂 Source: Filesystem directory './fingerprints'
🗄️  Destination: PostgreSQL from 'config.postgresql.toml'
📊 Loading fingerprints from source...
Found 150 files to migrate
  ✅ Migrated 'audio_001' (1234 fingerprints)
  ✅ Migrated 'audio_002' (987 fingerprints)
  ⏭️  Skipping 'audio_003' (already exists)
  ❌ Failed to migrate 'audio_004': Connection timeout
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📈 Migration Summary:
   Total files:    150
   ✅ Migrated:    147
   ⏭️  Skipped:     2
   ❌ Failed:      1
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ Migration completed successfully
```

## Error Handling

The tool handles various error scenarios:

- **Missing metadata**: Skips files without metadata
- **Connection errors**: Reports and continues with next file
- **Duplicate files**: Skips if `--skip-existing` is enabled
- **Invalid data**: Logs error and continues

Failed migrations are counted and reported in the summary. If any files fail, the tool exits with a non-zero status code.

## Performance Tips

1. **Connection Pool**: Adjust `max_connections` in PostgreSQL config for better performance
2. **Batch Size**: The tool uses batch insertion for fingerprints (optimized by default)
3. **Network**: Run migration on the same network as PostgreSQL for faster transfers
4. **Dry Run First**: Always run with `--dry-run` first to verify the migration plan

## Troubleshooting

### "Connection refused"
- Ensure PostgreSQL is running: `docker-compose ps`
- Check connection details in config file
- Verify firewall settings

### "File not found"
- Verify source directory path
- Check file permissions
- Ensure files have `.json` or `.bson` extension

### "Already exists" (when you want to re-migrate)
- Use `--skip-existing=false` flag
- Or delete existing data from PostgreSQL first

### Slow migration
- Increase `max_connections` in PostgreSQL config
- Check network latency
- Consider migrating in smaller batches

## See Also

- [DATABASE.md](../../DATABASE.md) - PostgreSQL setup and schema
- [config.postgresql.toml](../../config.postgresql.toml) - Example PostgreSQL configuration
- [docker-compose.yml](../../docker-compose.yml) - PostgreSQL Docker setup
