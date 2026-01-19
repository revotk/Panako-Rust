//! fpgen - Fingerprint generator with monitor mode support
//!
//! Usage: fpgen <input_audio_path> <output_dir>

use anyhow::{Context, Result};
use clap::Parser;
use panako_core::{
    audio::AudioData, config::PanakoConfig, eventpoint::EventPointExtractor,
    fingerprint::FingerprintGenerator, segmentation::{segment_audio, should_segment, SegmentationConfig},
    transform,
};
use panako_fp::{FpFile, FpHeader, FpMetadata, FpWriter, SegmentationInfo, SegmentMetadata};
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

    // Run fingerprint generation
    run_fpgen(&args.input_audio_path, &args.output_dir, args.monitor)?;

    Ok(())
}

fn run_fpgen(input_path: &str, output_dir: &str, use_monitor_mode: bool) -> Result<()> {
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

    // Create output filename
    let output_filename = input_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
        + ".fp";
    let output_path = output_dir.join(output_filename);

    // Prepare metadata
    let config_json = serde_json::to_string(&config)?;

    let metadata = FpMetadata {
        algorithm_id: "PANAKO".to_string(),
        algorithm_params: config_json.clone(),
        original_filename: input_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
        segmentation: segmentation_info,
    };

    // Calculate metadata size
    let metadata_json = serde_json::to_string(&metadata)?;
    let metadata_size = metadata_json.len() as u64;

    let header = FpHeader::new(
        metadata_size,
        (all_fingerprints.len() * 20) as u64,
        all_fingerprints.len() as u32,
        config.sample_rate,
        audio_data.duration_ms,
        1, // mono
    );

    // Convert fingerprints to tuple format
    let fp_data: Vec<(u64, i32, i16, f32)> = all_fingerprints
        .iter()
        .map(|fp| (fp.hash, fp.t1, fp.f1, fp.m1))
        .collect();

    let fp_file = FpFile {
        header,
        metadata,
        fingerprints: fp_data,
    };

    // Write .fp file
    let writer = FpWriter::new();
    writer.write(&output_path, &fp_file)?;

    // Print JSON output
    let mut result = serde_json::json!({
        "status": "success",
        "input_file": input_path.display().to_string(),
        "output_file": output_path.display().to_string(),
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
