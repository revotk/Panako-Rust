//! Panako Core - Acoustic Fingerprinting Library
//!
//! This crate implements the Panako acoustic fingerprinting algorithm,
//! ported from the Java reference implementation.

pub mod audio;
pub mod config;
pub mod eventpoint;
pub mod fingerprint;
pub mod matching;
pub mod transform;
pub mod segmentation;

pub use config::PanakoConfig;
pub use eventpoint::{EventPoint, EventPointExtractor};
pub use fingerprint::{Fingerprint, FingerprintGenerator};
pub use matching::{Matcher, QueryResult};
pub use segmentation::{segment_audio, should_segment, AudioSegment, SegmentationConfig};

/// Generate fingerprints from audio file
pub fn generate_fingerprints(
    audio_path: &str,
    config: &PanakoConfig,
) -> anyhow::Result<Vec<Fingerprint>> {
    // Decode audio
    let audio_data = audio::decode_audio(audio_path, config.sample_rate)?;
    
    // Convert to mono samples
    let mono_samples = audio_data.to_mono();
    
    // Extract spectral representation
    let spectrogram = transform::compute_transform(&mono_samples, config)?;
    
    // Extract event points
    let event_points = EventPointExtractor::new(config).extract(&spectrogram)?;
    
    // Generate fingerprints
    let fingerprints = FingerprintGenerator::new(config).generate(&event_points)?;
    
    Ok(fingerprints)
}
