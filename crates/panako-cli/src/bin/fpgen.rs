//! fpgen - Fingerprint generator with monitor mode support
//!
//! Usage: fpgen <input_audio_path> <output_dir>

use anyhow::{Context, Result};
use clap::Parser;
use panako_core::{
    audio::AudioData,
    config::PanakoConfig,
    eventpoint::EventPointExtractor,
    fingerprint::FingerprintGenerator,
    segmentation::{segment_audio, should_segment, SegmentationConfig},
    storage_config::{FileFormat, PanakoStorageConfig},
    transform,
};
use panako_fp::{FpJsonFile, FpJsonFingerprint, FpJsonSegment, SegmentationInfo, SegmentMetadata};
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "fpgen")]
#[command(about = "Generate Panako fingerprints from audio files", long_about = None)]
struct Args {
    /// Input audio file path
    input_audio_path: String,

    /// Output directory for .fp files
    output_dir: String,

    /// Enable monitor mode (segment files >25s with 5s overlap)
    #[arg(short, long)]
    monitor: bool,

    /// Path to configuration file (TOML)
    #[arg(short, long)]
    config: Option<String>,

    /// Override output format (json or bson)
    #[arg(long)]
    format: Option<String>,

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

    // Determine format from args or config
    let mut format = FileFormat::Json; // Default
    
    // 1. Check config file first
    if let Some(config_path) = &args.config {
        if let Ok(config) = PanakoStorageConfig::load(Path::new(config_path)) {
            format = config.storage.filesystem.format;
        } else {
            log::warn!("Failed to load config file, using defaults");
        }
    } else {
        // Try default config.toml if exists
        if Path::new("config.toml").exists() {
             if let Ok(config) = PanakoStorageConfig::load(Path::new("config.toml")) {
                format = config.storage.filesystem.format;
            }
        }
    }

    // 2. Override with CLI argument if provided
    if let Some(fmt_str) = &args.format {
        format = match fmt_str.to_lowercase().as_str() {
            "json" => FileFormat::Json,
            "bson" => FileFormat::Bson,
            _ => {
                log::warn!("Unknown format '{}', defaulting to JSON", fmt_str);
                FileFormat::Json
            }
        };
    }

    // If format is Auto, default to JSON for generation (safer default)
    if format == FileFormat::Auto {
        format = FileFormat::Json;
    }

    // Run fingerprint generation
    run_fpgen(&args.input_audio_path, &args.output_dir, args.monitor, format)?;

    Ok(())
}

fn run_fpgen(
    input_path: &str, 
    output_dir: &str, 
    use_monitor_mode: bool,
    format: FileFormat
) -> Result<()> {
    let input_path = Path::new(input_path);
    let output_dir = Path::new(output_dir);

    // Validate input
    if !input_path.exists() {
        anyhow::bail!("Input file not found: {}", input_path.display());
    }

    // Create output directory if needed
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    // Load configuration
    let config = PanakoConfig::default();
    config.validate()?;

    log::info!("Processing: {}", input_path.display());

    // Decode audio
    let start = std::time::Instant::now();
    let audio_data = panako_core::audio::decode_audio(
        input_path.to_str().unwrap(),
        config.sample_rate,
    )?;
    let _decode_time = start.elapsed();

    log::info!(
        "Decoded audio: {:.1}s duration, {} samples @ {}Hz",
        audio_data.duration_ms as f64 / 1000.0,
        audio_data.samples.len(),
        audio_data.sample_rate
    );

    // Check if monitor mode is enabled and segmentation is needed
    let seg_config = SegmentationConfig::default();
    let use_segmentation = use_monitor_mode && should_segment(&audio_data, &seg_config);

    let (all_fingerprints, segmentation_info, total_segments) = if use_segmentation {
        log::info!(
            "Monitor mode enabled - Segmenting audio ({:.1}s) into {}s chunks with {}s overlap",
            audio_data.duration_ms as f64 / 1000.0,
            seg_config.segment_duration_s,
            seg_config.overlap_duration_s
        );

        process_with_segmentation(&audio_data, &config, &seg_config)?
    } else {
        if use_monitor_mode {
            log::info!(
                "Monitor mode enabled but audio duration {:.1}s <= {}s - Using normal mode",
                audio_data.duration_ms as f64 / 1000.0,
                seg_config.segment_duration_s
            );
        } else {
            log::info!("Normal mode - Processing as single file");
        }

        let fingerprints = generate_fingerprints_from_audio(&audio_data, &config)?;
        (fingerprints, None, 1)
    };

    let elapsed = start.elapsed();

    log::info!(
        "Generated {} fingerprints in {:.2}s ({} segments)",
        all_fingerprints.len(),
        elapsed.as_secs_f64(),
        total_segments
    );

    // Extract filename without extension
    let filename = input_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Create output filename based on format
    let ext = match format {
        FileFormat::Bson => "bson",
        _ => "json",
    };
    let output_filename = format!("{}.{}", filename, ext);
    let output_path = output_dir.join(output_filename);

    // Create fingerprint file object
    let mut fp_file = FpJsonFile::new(
        input_path.to_str().unwrap().to_string(),
        filename,
        config.sample_rate,
        audio_data.duration_ms,
        1, // mono
    );

    // Add segmentation info if applicable
    if use_segmentation {
        fp_file = fp_file.with_segmentation(
            seg_config.segment_duration_s,
            seg_config.overlap_duration_s,
            total_segments,
        );
    }

    // Build segments
    if let Some(seg_info) = &segmentation_info {
        // Multiple segments
        for seg_meta in &seg_info.segments {
            let start_idx = seg_meta.fingerprint_offset as usize;
            let end_idx = start_idx + seg_meta.num_fingerprints as usize;
            
            let segment = FpJsonSegment {
                segment_id: seg_meta.segment_id,
                start_time_s: seg_meta.start_time_ms as f64 / 1000.0,
                end_time_s: seg_meta.end_time_ms as f64 / 1000.0,
                num_fingerprints: seg_meta.num_fingerprints as usize,
                fingerprints: all_fingerprints[start_idx..end_idx]
                    .iter()
                    .map(|fp| FpJsonFingerprint {
                        hash: fp.hash,
                        t1: fp.t1,
                        f1: fp.f1,
                        m1: fp.m1,
                    })
                    .collect(),
            };
            
            fp_file.add_segment(segment);
        }
    } else {
        // Single segment
        let end_time_s = all_fingerprints
            .last()
            .map(|fp| fp.t1 as f64 * 0.008)
            .unwrap_or(audio_data.duration_ms as f64 / 1000.0);
        
        let segment = FpJsonSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s,
            num_fingerprints: all_fingerprints.len(),
            fingerprints: all_fingerprints
                .iter()
                .map(|fp| FpJsonFingerprint {
                    hash: fp.hash,
                    t1: fp.t1,
                    f1: fp.f1,
                    m1: fp.m1,
                    })
                .collect(),
        };
        
        fp_file.add_segment(segment);
    }

    // Save file based on format
    match format {
        FileFormat::Bson => fp_file.save_bson(&output_path)?,
        _ => fp_file.save(&output_path)?,
    }

    // Print JSON output for CLI (still returning JSON status)
    let mut result = serde_json::json!({
        "status": "success",
        "input_file": input_path.display().to_string(),
        "output_file": output_path.display().to_string(),
        "format": ext,
        "num_fingerprints": all_fingerprints.len(),
        "processing_time_seconds": elapsed.as_secs_f64(),
    });

    if use_segmentation {
        result["num_segments"] = total_segments.into();
        result["segment_duration_s"] = seg_config.segment_duration_s.into();
        result["overlap_duration_s"] = seg_config.overlap_duration_s.into();
    }

    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}


/// Process audio with segmentation (monitor mode)
fn process_with_segmentation(
    audio_data: &AudioData,
    config: &PanakoConfig,
    seg_config: &SegmentationConfig,
) -> Result<(Vec<panako_core::Fingerprint>, Option<SegmentationInfo>, usize)> {
    // Segment the audio
    let segments = segment_audio(audio_data, seg_config);
    log::info!(
        "Created {} segments with {}s overlap",
        segments.len(),
        seg_config.overlap_duration_s
    );

    let mut all_fingerprints = Vec::new();
    let mut segment_metadata = Vec::new();

    for segment in &segments {
        log::debug!(
            "Processing segment {}: {:.1}s - {:.1}s ({:.1}s duration)",
            segment.segment_id,
            segment.start_time_s,
            segment.end_time_s,
            segment.end_time_s - segment.start_time_s
        );

        // Create AudioData for this segment
        let segment_audio = AudioData {
            samples: segment.samples.clone(),
            sample_rate: segment.sample_rate,
            channels: 1,
            duration_ms: ((segment.end_time_s - segment.start_time_s) * 1000.0) as u32,
        };

        // Generate fingerprints for this segment
        let segment_fps = generate_fingerprints_from_audio(&segment_audio, config)?;

        // Adjust timestamps to absolute time
        let time_offset_frames = (segment.start_time_s / 0.008) as i32;
        let adjusted_fps: Vec<_> = segment_fps
            .iter()
            .map(|fp| {
                let mut adjusted = fp.clone();
                adjusted.t1 += time_offset_frames;
                adjusted.t2 += time_offset_frames;
                adjusted.t3 += time_offset_frames;
                adjusted
            })
            .collect();

        log::debug!(
            "  Generated {} fingerprints (offset: {} frames)",
            adjusted_fps.len(),
            time_offset_frames
        );

        // Store segment metadata
        segment_metadata.push(SegmentMetadata {
            segment_id: segment.segment_id,
            start_time_ms: (segment.start_time_s * 1000.0) as u32,
            end_time_ms: (segment.end_time_s * 1000.0) as u32,
            num_fingerprints: adjusted_fps.len() as u32,
            fingerprint_offset: all_fingerprints.len() as u32,
        });

        all_fingerprints.extend(adjusted_fps);
    }

    let segmentation_info = SegmentationInfo {
        num_segments: segments.len(),
        segment_duration_ms: (seg_config.segment_duration_s * 1000.0) as u32,
        overlap_duration_ms: (seg_config.overlap_duration_s * 1000.0) as u32,
        segments: segment_metadata,
    };

    Ok((all_fingerprints, Some(segmentation_info), segments.len()))
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
