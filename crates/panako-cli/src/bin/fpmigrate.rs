//! Migration tool for transferring fingerprints from filesystem to PostgreSQL
//!
//! Usage:
//!   fpmigrate --source-dir ./fingerprints --config config.postgresql.toml
//!   fpmigrate --source-config config.toml --dest-config config.postgresql.toml

use anyhow::{Context, Result};
use clap::Parser;
use panako_core::{
    storage_backend::{FilesystemBackend, PostgresqlBackend, StorageBackend},
    storage_config::{FileFormat, FilesystemConfig, PanakoStorageConfig, StorageBackend as BackendType},
};
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "fpmigrate")]
#[command(about = "Migrate fingerprints from filesystem to PostgreSQL", long_about = None)]
struct Args {
    /// Source directory containing fingerprint files (JSON/BSON)
    #[arg(long, conflicts_with = "source_config")]
    source_dir: Option<String>,

    /// Source configuration file (filesystem backend)
    #[arg(long, conflicts_with = "source_dir")]
    source_config: Option<String>,

    /// Destination configuration file (PostgreSQL backend)
    #[arg(long, required = true)]
    dest_config: String,

    /// Dry run - show what would be migrated without actually migrating
    #[arg(long, default_value = "false")]
    dry_run: bool,

    /// Skip files that already exist in destination
    #[arg(long, default_value = "true")]
    skip_existing: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("🚀 Starting fingerprint migration");

    // Create source backend
    let source_backend = create_source_backend(&args)?;
    
    // Create destination backend
    let dest_backend = create_dest_backend(&args).await?;

    // Perform migration
    migrate_fingerprints(
        source_backend.as_ref(),
        dest_backend.as_ref(),
        args.dry_run,
        args.skip_existing,
    )
    .await?;

    log::info!("✅ Migration completed successfully");

    Ok(())
}

fn create_source_backend(args: &Args) -> Result<Box<dyn StorageBackend>> {
    if let Some(source_dir) = &args.source_dir {
        log::info!("📂 Source: Filesystem directory '{}'", source_dir);
        let config = FilesystemConfig {
            base_directory: source_dir.clone(),
            format: FileFormat::Auto,
        };
        Ok(Box::new(FilesystemBackend::new(&config)))
    } else if let Some(source_config) = &args.source_config {
        log::info!("📂 Source: Configuration file '{}'", source_config);
        let config = PanakoStorageConfig::load(Path::new(source_config))
            .context("Failed to load source configuration")?;
        
        match config.storage.backend {
            BackendType::Filesystem => {
                Ok(Box::new(FilesystemBackend::new(&config.storage.filesystem)))
            }
            BackendType::Postgresql => {
                anyhow::bail!("Source backend must be filesystem, not PostgreSQL")
            }
        }
    } else {
        anyhow::bail!("Either --source-dir or --source-config must be provided")
    }
}

async fn create_dest_backend(args: &Args) -> Result<Box<dyn StorageBackend>> {
    log::info!("🗄️  Destination: PostgreSQL from '{}'", args.dest_config);
    
    let config = PanakoStorageConfig::load(Path::new(&args.dest_config))
        .context("Failed to load destination configuration")?;
    
    match config.storage.backend {
        BackendType::Postgresql => {
            let backend = PostgresqlBackend::new(&config.storage.postgresql)
                .await
                .context("Failed to create PostgreSQL backend")?;
            Ok(Box::new(backend))
        }
        BackendType::Filesystem => {
            anyhow::bail!("Destination backend must be PostgreSQL, not filesystem")
        }
    }
}

async fn migrate_fingerprints(
    source: &dyn StorageBackend,
    dest: &dyn StorageBackend,
    dry_run: bool,
    skip_existing: bool,
) -> Result<()> {
    log::info!("📊 Loading fingerprints from source...");
    
    // Load all fingerprints from source
    let all_fingerprints = source
        .load_all_fingerprints()
        .await
        .context("Failed to load fingerprints from source")?;
    
    let total_files = all_fingerprints.len();
    log::info!("Found {} files to migrate", total_files);

    let mut migrated = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for (identifier, fingerprints) in all_fingerprints {
        log::debug!("Processing '{}'...", identifier);

        // Check if already exists in destination
        if skip_existing {
            match dest.get_metadata(&identifier).await {
                Ok(Some(_)) => {
                    log::debug!("  ⏭️  Skipping '{}' (already exists)", identifier);
                    skipped += 1;
                    continue;
                }
                Ok(None) => {
                    // Doesn't exist, continue with migration
                }
                Err(e) => {
                    log::warn!("  ⚠️  Error checking existence of '{}': {}", identifier, e);
                    // Continue anyway
                }
            }
        }

        if dry_run {
            log::info!(
                "  [DRY RUN] Would migrate '{}' ({} fingerprints)",
                identifier,
                fingerprints.len()
            );
            migrated += 1;
            continue;
        }

        // Get metadata from source
        let metadata = match source.get_metadata(&identifier).await {
            Ok(Some(meta)) => meta,
            Ok(None) => {
                log::warn!("  ⚠️  No metadata found for '{}', skipping", identifier);
                failed += 1;
                continue;
            }
            Err(e) => {
                log::error!("  ❌ Failed to get metadata for '{}': {}", identifier, e);
                failed += 1;
                continue;
            }
        };

        // Save to destination
        match dest
            .save_fingerprints(&identifier, &fingerprints, &metadata)
            .await
        {
            Ok(_) => {
                log::info!(
                    "  ✅ Migrated '{}' ({} fingerprints)",
                    identifier,
                    fingerprints.len()
                );
                migrated += 1;
            }
            Err(e) => {
                log::error!("  ❌ Failed to migrate '{}': {}", identifier, e);
                failed += 1;
            }
        }
    }

    // Print summary
    log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    log::info!("📈 Migration Summary:");
    log::info!("   Total files:    {}", total_files);
    log::info!("   ✅ Migrated:    {}", migrated);
    log::info!("   ⏭️  Skipped:     {}", skipped);
    log::info!("   ❌ Failed:      {}", failed);
    log::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if failed > 0 {
        anyhow::bail!("{} files failed to migrate", failed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        // Test with source-dir
        let args = Args::parse_from(&[
            "fpmigrate",
            "--source-dir",
            "./fingerprints",
            "--dest-config",
            "config.postgresql.toml",
        ]);
        assert_eq!(args.source_dir, Some("./fingerprints".to_string()));
        assert_eq!(args.dest_config, "config.postgresql.toml");

        // Test with source-config
        let args = Args::parse_from(&[
            "fpmigrate",
            "--source-config",
            "config.toml",
            "--dest-config",
            "config.postgresql.toml",
        ]);
        assert_eq!(args.source_config, Some("config.toml".to_string()));
    }
}
