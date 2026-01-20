//! fpmonitor - Monitor mode for long audio/video files
//!
//! Processes long media files in 25-second segments with 5-second overlap,
//! generating fingerprints on-the-fly and matching against a database.
//!
//! Usage: fpmonitor <db_dir> <input_file>

use anyhow::Result;
use clap::Parser;
use panako_cli::output::print_json_results;
use panako_core::{
    audio::AudioData, config::PanakoConfig, eventpoint::EventPointExtractor,
    fingerprint::FingerprintGenerator, matching::{Matcher, QueryResult},
    segmentation::{segment_audio, SegmentationConfig}, transform,
};
use panako_fp::FpJsonFile;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "fpmonitor")]
#[command(about = "Monitor long audio/video files for matches", long_about = None)]
struct Args {
    /// Database directory containing .fp files
    db_dir: String,

    /// Input video/audio file (.ts, .mp4, .mp3, etc.)
    input_file: String,

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

    // Run monitor
    run_fpmonitor(&args.db_dir, &args.input_file)?;

    Ok(())
}

fn run_fpmonitor(db_dir: &str, input_file: &str) -> Result<()> {
    let db_path = Path::new(db_dir);
    let input_path = Path::new(input_file);

    // Validate paths
    if !db_path.exists() {
        anyhow::bail!("Database directory not found: {}", db_path.display());
    }
    if !input_path.exists() {
        anyhow::bail!("Input file not found: {}", input_path.display());
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

    log::info!("Found {} .json/.bson files, loading in parallel...", fp_files.len());

    // Load all files in parallel
    let load_start = std::time::Instant::now();
    let loaded_files: Vec<(String, FpJsonFile)> = fp_files
        .par_iter()
        .filter_map(|path| {
            log::debug!("Loading: {}", path.display());
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
        let all_fps = fp_file.get_all_fingerprints();
        matcher.add_fingerprints(identifier.clone(), &all_fps);
        matcher.add_duration(identifier, fp_file.metadata.duration_ms);
    }

    log::info!("Processing input file: {}", input_path.display());

    // Load configuration
    let config = PanakoConfig::default();
    config.validate()?;

    // Decode entire audio file
    let decode_start = std::time::Instant::now();
    let audio_data = panako_core::audio::decode_audio(
        input_path.to_str().unwrap(),
        config.sample_rate,
    )?;
    let decode_duration = decode_start.elapsed();

    log::info!(
        "Decoded audio: {:.1}s duration, {} samples @ {}Hz (took {:.2}s)",
        audio_data.duration_ms as f64 / 1000.0,
        audio_data.samples.len(),
        audio_data.sample_rate,
        decode_duration.as_secs_f64()
    );

    // Segment audio
    let seg_config = SegmentationConfig::default();
    let segments = segment_audio(&audio_data, &seg_config);

    log::info!(
        "Segmented into {} segments ({}s duration, {}s overlap)",
        segments.len(),
        seg_config.segment_duration_s,
        seg_config.overlap_duration_s
    );

    // Process each segment
    let mut all_results = Vec::new();
    let process_start = std::time::Instant::now();

    for (idx, segment) in segments.iter().enumerate() {
        log::info!(
            "Processing segment {}/{}: {:.1}s - {:.1}s",
            idx + 1,
            segments.len(),
            segment.start_time_s,
            segment.end_time_s
        );

        // Process segment and query
        let mut segment_results = process_segment_and_query(
            segment,
            &matcher,
            &config,
            input_path.to_str().unwrap(),
        )?;

        // Add segment info to results
        for res in &mut segment_results {
            res.segment_index = Some(idx);
        }

        log::info!(
            "  Segment {} found {} matches",
            idx + 1,
            segment_results.len()
        );

        all_results.extend(segment_results);
    }

    let process_duration = process_start.elapsed();
    log::info!(
        "Processed {} segments in {:.2}s",
        segments.len(),
        process_duration.as_secs_f64()
    );

    // Sort results by absolute start time
    all_results.sort_by(|a, b| {
        let a_start = a.absolute_start.unwrap_or(a.query_start);
        let b_start = b.absolute_start.unwrap_or(b.query_start);
        a_start.partial_cmp(&b_start).unwrap_or(std::cmp::Ordering::Equal)
    });

    log::info!(
        "Final results: {} detections (per-segment reporting)",
        all_results.len()
    );

    // Print results
    print_json_results(&all_results);

    Ok(())
}

/// Process a single segment: generate fingerprints and query matcher
fn process_segment_and_query(
    segment: &panako_core::segmentation::AudioSegment,
    matcher: &Matcher,
    config: &PanakoConfig,
    query_path: &str,
) -> Result<Vec<QueryResult>> {
    // Create AudioData for this segment
    let segment_audio = AudioData {
        samples: segment.samples.clone(),
        sample_rate: segment.sample_rate,
        channels: 1,
        duration_ms: ((segment.end_time_s - segment.start_time_s) * 1000.0) as u32,
    };

    // Generate fingerprints
    let fingerprints = generate_fingerprints_from_audio(&segment_audio, config)?;

    if fingerprints.is_empty() {
        return Ok(vec![]);
    }

    // Adjust timestamps to absolute time (relative to full file)
    let time_offset_frames = (segment.start_time_s / 0.008) as i32;
    let adjusted_fps: Vec<_> = fingerprints
        .iter()
        .map(|fp| {
            let mut adjusted = fp.clone();
            adjusted.t1 += time_offset_frames;
            adjusted.t2 += time_offset_frames;
            adjusted.t3 += time_offset_frames;
            adjusted
        })
        .collect();

    // Convert to tuple format for matcher
    let fp_tuples: Vec<(u64, i32, i16, f32)> = adjusted_fps
        .iter()
        .map(|fp| (fp.hash, fp.t1, fp.f1, fp.m1))
        .collect();

    // Query matcher
    let results = matcher.query(query_path, &fp_tuples, config)?;

    // Adjust query result times (they're already absolute due to adjusted fingerprints)
    // No additional adjustment needed since we adjusted the fingerprints before querying

    Ok(results)
}

/// Generate fingerprints from audio data
fn generate_fingerprints_from_audio(
    audio: &AudioData,
    config: &PanakoConfig,
) -> Result<Vec<panako_core::Fingerprint>> {
    // Convert to mono
    let mono_samples = audio.to_mono();

    // Compute spectral transform
    let spectrogram = transform::compute_transform(&mono_samples, config)?;

    // Extract event points
    let event_points = EventPointExtractor::new(config).extract(&spectrogram)?;

    // Generate fingerprints
    let fingerprints = FingerprintGenerator::new(config).generate(&event_points)?;

    Ok(fingerprints)
}


