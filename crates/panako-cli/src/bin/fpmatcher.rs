//! fpmatcher - Fingerprint matcher
//!
//! Usage: 
//!   fpmatcher <query_fp>                    # Uses config.toml
//!   fpmatcher --config <path> <query_fp>    # Uses custom config
//!   fpmatcher <db_dir> <query_fp>           # Legacy mode (filesystem)

use anyhow::Result;
use clap::Parser;
use panako_cli::output::print_json_results;
use panako_core::matching::Matcher;
use panako_core::{PanakoStorageConfig, StorageBackend};
use panako_fp::FpJsonFile;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "fpmatcher")]
#[command(about = "Match fingerprints against a database", long_about = None)]
struct Args {
    /// Path to configuration file (TOML). If not provided, uses config.toml
    #[arg(short, long)]
    config: Option<String>,

    /// Database directory (legacy mode, overrides config if provided)
    /// OR query fingerprint file if using config mode
    first_arg: String,

    /// Query fingerprint file (only used in legacy mode)
    second_arg: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Off)
            .init();
    }

    // Determine mode: config-based or legacy
    let (db_dir, query_fp) = if let Some(second_arg) = &args.second_arg {
        // Legacy mode: fpmatcher <db_dir> <query_fp>
        log::info!("Running in legacy mode (filesystem)");
        (Some(args.first_arg.clone()), second_arg.clone())
    } else {
        // Config mode: fpmatcher [--config <path>] <query_fp>
        log::info!("Running in config mode");
        (None, args.first_arg.clone())
    };

    // Run matching
    if let Some(db_dir) = db_dir {
        // Legacy mode: use filesystem directly
        run_fpmatcher(&db_dir, &query_fp)?;
    } else {
        // Config mode: load config and use appropriate backend
        let config_path = args.config.as_deref().unwrap_or("config.toml");
        run_fpmatcher_with_config(config_path, &query_fp)?;
    }

    Ok(())
}

fn run_fpmatcher(db_dir: &str, query_fp: &str) -> Result<()> {
    let db_path = Path::new(db_dir);
    let query_path = Path::new(query_fp);

    // Validate paths
    if !db_path.exists() {
        anyhow::bail!("Database directory not found: {}", db_path.display());
    }
    if !query_path.exists() {
        anyhow::bail!("Query file not found: {}", query_path.display());
    }

    log::info!("Loading database from: {}", db_path.display());

    // Find all .json and .bson files in database directory
    let fp_files: Vec<PathBuf> = std::fs::read_dir(db_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext == "json" || ext == "bson")
                .unwrap_or(false)
        })
        .collect();

    log::info!("Found {} fingerprint files, loading in parallel...", fp_files.len());

    // Load all files in parallel
    let load_start = std::time::Instant::now();
    let loaded_files: Vec<(String, FpJsonFile)> = fp_files
        .par_iter()
        .filter_map(|path| {
            log::debug!("Loading: {}", path.display());
            // Use load_auto to handle both JSON and BSON automatically
            match FpJsonFile::load_auto(path) {
                Ok(fp_file) => {
                    let identifier = fp_file.metadata.filename.clone();
                    Some((identifier, fp_file))
                }
                Err(e) => {
                    log::warn!("Failed to load {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    let load_duration = load_start.elapsed();
    log::info!(
        "Loaded {} files in {:.2}s ({:.0} files/sec)",
        loaded_files.len(),
        load_duration.as_secs_f64(),
        loaded_files.len() as f64 / load_duration.as_secs_f64()
    );

    // Build matcher
    let mut matcher = Matcher::new();
    for (identifier, fp_file) in loaded_files {
        // Get all fingerprints from all segments
        let all_fps = fp_file.get_all_fingerprints();
        matcher.add_fingerprints(identifier.clone(), &all_fps);
        // Store reference duration
        matcher.add_duration(identifier, fp_file.metadata.duration_ms);
    }

    // Load query
    log::info!("Loading query: {}", query_path.display());
    let query_file = FpJsonFile::load_auto(query_path)?;
    let query_fps = query_file.get_all_fingerprints();
    log::info!("Query has {} fingerprints", query_fps.len());

    // Perform matching (per segment if available)
    let match_start = std::time::Instant::now();
    let config = panako_core::config::PanakoConfig::default();
    
    let mut results = Vec::new();
    
    if query_file.segments.len() > 1 {
        log::info!("Query file has {} segments, processing individually...", query_file.segments.len());
        for segment in &query_file.segments {
            let seg_fps: Vec<_> = segment.fingerprints.iter().map(|fp| (fp.hash, fp.t1, fp.f1, fp.m1)).collect();
            let mut seg_results = matcher.query(
                query_path.to_str().unwrap(),
                &seg_fps,
                &config,
            )?;
            
            // Add segment info
            for res in &mut seg_results {
                res.segment_index = Some(segment.segment_id);
            }
            results.extend(seg_results);
        }
    } else {
        let query_fps = query_file.get_all_fingerprints();
        let query_results = matcher.query(
            query_path.to_str().unwrap(),
            &query_fps,
            &config,
        )?;
        results.extend(query_results);
    }
    let match_duration = match_start.elapsed();

    log::info!(
        "Matching completed in {:.2}s, found {} results",
        match_duration.as_secs_f64(),
        results.len()
    );

    // Print results
    print_json_results(&results);

    Ok(())
}

/// Config-based matching (supports filesystem or PostgreSQL)
fn run_fpmatcher_with_config(config_path: &str, query_fp: &str) -> Result<()> {
    // Load configuration
    let config = PanakoStorageConfig::load(Path::new(config_path))?;
    
    log::info!("Loaded configuration from: {}", config_path);
    log::info!("Storage backend: {:?}", config.storage.backend);
    
    match config.storage.backend {
        StorageBackend::Filesystem => {
            // Use filesystem backend
            let db_dir = &config.storage.filesystem.base_directory;
            log::info!("Using filesystem backend: {}", db_dir);
            run_fpmatcher(db_dir, query_fp)
        }
        StorageBackend::Postgresql => {
            // TODO: Implement PostgreSQL backend matching
            anyhow::bail!("PostgreSQL backend not yet implemented. Please use filesystem backend for now.")
        }
    }
}
