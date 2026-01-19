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
use std::collections::HashMap;
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

    // Find all .json files in database directory
    let fp_files: Vec<PathBuf> = std::fs::read_dir(db_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();

    log::info!("Found {} .json files, loading in parallel...", fp_files.len());

    // Load all files in parallel
    let load_start = std::time::Instant::now();
    let loaded_files: Vec<(String, FpJsonFile)> = fp_files
        .par_iter()
        .filter_map(|path| {
            log::debug!("Loading: {}", path.display());
            match FpJsonFile::load(path) {
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
        let segment_results = process_segment_and_query(
            segment,
            &matcher,
            &config,
            input_path.to_str().unwrap(),
        )?;

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

    // Merge overlapping detections
    log::info!("Merging overlapping detections...");
    let merged_results = merge_overlapping_detections(all_results);

    log::info!(
        "Final results: {} detections after merging",
        merged_results.len()
    );

    // Print results
    print_json_results(&merged_results);

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
    let results = matcher.query(query_path, &fp_tuples)?;

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

/// Merge overlapping detections from different segments
/// 
/// Uses dynamic threshold based on reference duration (1/3 of duration).
/// For detections with absolute_start difference < threshold:
/// - Keep the one with better score
/// - If same score, keep the first detection (chronologically)
fn merge_overlapping_detections(results: Vec<QueryResult>) -> Vec<QueryResult> {
    if results.is_empty() {
        return vec![];
    }

    // Group by ref_identifier
    let mut by_ref: HashMap<String, Vec<QueryResult>> = HashMap::new();

    for result in results {
        if let Some(ref_id) = &result.ref_identifier {
            by_ref
                .entry(ref_id.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
    }

    let mut merged = Vec::new();

    for (_, mut group) in by_ref {
        // Sort by absolute_start (chronological order)
        // This ensures we process detections in the order they appear in the file
        group.sort_by(|a, b| {
            let a_abs = a.absolute_start.unwrap_or(a.query_start);
            let b_abs = b.absolute_start.unwrap_or(b.query_start);
            a_abs
                .partial_cmp(&b_abs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate based on dynamic threshold
        let mut i = 0;
        while i < group.len() {
            let current = &group[i];
            
            // Calculate dynamic threshold: 1/3 of reference duration
            let ref_duration_s = if let Some(duration_ms) = current.ref_duration_ms {
                duration_ms as f64 / 1000.0
            } else {
                // Fallback: use detected duration
                current.ref_stop - current.ref_start
            };
            
            let threshold = ref_duration_s / 3.0;
            
            log::debug!(
                "Processing detection at {:.2}s (ref: {:?}, duration: {:.2}s, threshold: {:.2}s)",
                current.absolute_start.unwrap_or(current.query_start),
                current.ref_identifier,
                ref_duration_s,
                threshold
            );

            // Find duplicates (detections that start within threshold)
            let mut duplicates = vec![i];
            let current_abs_start = current.absolute_start.unwrap_or(current.query_start);
            
            for j in (i + 1)..group.len() {
                let next = &group[j];
                let next_abs_start = next.absolute_start.unwrap_or(next.query_start);
                
                let time_diff = (next_abs_start - current_abs_start).abs();
                
                if time_diff < threshold {
                    log::debug!(
                        "  Found potential duplicate at {:.2}s (diff: {:.2}s < {:.2}s)",
                        next_abs_start,
                        time_diff,
                        threshold
                    );
                    duplicates.push(j);
                } else {
                    // Since sorted, no more duplicates possible
                    break;
                }
            }

            // Select best detection from duplicates
            let best_idx = if duplicates.len() > 1 {
                // Multiple detections within threshold - select best
                let mut best = duplicates[0];
                let mut best_score = group[best].score;
                
                for &idx in &duplicates[1..] {
                    let score = group[idx].score;
                    if score > best_score {
                        best = idx;
                        best_score = score;
                    }
                    // If same score, keep the first one (already in 'best')
                }
                
                log::debug!(
                    "  Selected detection at index {} with score {} (from {} duplicates)",
                    best,
                    best_score,
                    duplicates.len()
                );
                
                best
            } else {
                duplicates[0]
            };

            // Add the best detection to results
            merged.push(group[best_idx].clone());

            // Skip all duplicates
            i = duplicates.last().unwrap() + 1;
        }
    }

    // Sort final results by absolute_start (chronological order)
    merged.sort_by(|a, b| {
        let a_abs = a.absolute_start.unwrap_or(a.query_start);
        let b_abs = b.absolute_start.unwrap_or(b.query_start);
        a_abs
            .partial_cmp(&b_abs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    merged
}
