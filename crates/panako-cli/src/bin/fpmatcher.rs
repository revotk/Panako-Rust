//! fpmatcher - Fingerprint matcher
//!
//! Usage: fpmatcher <db_dir> <query_fp>

use anyhow::Result;
use clap::Parser;
use panako_cli::output::print_json_results;
use panako_core::matching::Matcher;
use panako_fp::{FpReader, FpFile};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "fpmatcher")]
#[command(about = "Match fingerprints against a database", long_about = None)]
struct Args {
    /// Database directory containing .fp files
    db_dir: String,

    /// Query fingerprint file
    query_fp: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    // Default: no logs (clean JSON output for parsing)
    // Verbose: show Info level logs for debugging
    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Off)
            .init();
    }

    // Run matching
    run_fpmatcher(&args.db_dir, &args.query_fp)?;

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

    // Find all .fp files in database directory
    let fp_files: Vec<PathBuf> = std::fs::read_dir(db_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("fp"))
        .collect();

    log::info!("Found {} .fp files, loading in parallel...", fp_files.len());

    // Load all files in parallel
    let load_start = std::time::Instant::now();
    let loaded_files: Vec<(String, FpFile)> = fp_files
        .par_iter()
        .filter_map(|path| {
            log::debug!("Loading: {}", path.display());
            match FpReader::read(path) {
                Ok(fp_file) => {
                    let identifier = path
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
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
        matcher.add_fingerprints(identifier.clone(), &fp_file.fingerprints);
        // Store reference duration
        matcher.add_duration(identifier, fp_file.header.duration_ms);
    }

    // Load query
    log::info!("Loading query: {}", query_path.display());
    let query_file = FpReader::read(query_path)?;
    log::info!("Query has {} fingerprints", query_file.fingerprints.len());

    // Perform matching
    let match_start = std::time::Instant::now();
    let results = matcher.query(
        query_path.to_str().unwrap(),
        &query_file.fingerprints,
    )?;
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
