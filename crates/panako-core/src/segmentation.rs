//! Audio segmentation for monitor mode
//!
//! Implements automatic segmentation of long audio files into overlapping chunks
//! using Java Panako's default parameters (25s segments with 5s overlap).

use crate::audio::AudioData;

/// Configuration for audio segmentation
#[derive(Debug, Clone)]
pub struct SegmentationConfig {
    /// Duration of each segment in seconds
    pub segment_duration_s: f64,
    /// Overlap between segments in seconds
    pub overlap_duration_s: f64,
    /// Minimum duration for the last segment
    pub min_segment_duration_s: f64,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            segment_duration_s: 25.0,  // Java Panako default
            overlap_duration_s: 5.0,   // Java Panako default
            min_segment_duration_s: 10.0,
        }
    }
}

/// Represents a segment of audio
#[derive(Debug, Clone)]
pub struct AudioSegment {
    /// Segment identifier (0-based)
    pub segment_id: usize,
    /// Start time in seconds
    pub start_time_s: f64,
    /// End time in seconds
    pub end_time_s: f64,
    /// Audio samples for this segment
    pub samples: Vec<f32>,
    /// Sample rate
    pub sample_rate: u32,
}

/// Check if audio should be segmented based on duration
pub fn should_segment(audio: &AudioData, config: &SegmentationConfig) -> bool {
    let duration_s = audio.duration_ms as f64 / 1000.0;
    duration_s > config.segment_duration_s
}

/// Segment audio into overlapping chunks
pub fn segment_audio(
    audio: &AudioData,
    config: &SegmentationConfig,
) -> Vec<AudioSegment> {
    let duration_s = audio.duration_ms as f64 / 1000.0;
    
    if !should_segment(audio, config) {
        // No segmentation needed, return entire audio as single segment
        return vec![AudioSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s: duration_s,
            samples: audio.samples.clone(),
            sample_rate: audio.sample_rate,
        }];
    }
    
    let mut segments = Vec::new();
    let samples_per_second = audio.sample_rate as f64;
    let step_duration_s = config.segment_duration_s - config.overlap_duration_s;
    
    let mut current_start_s = 0.0;
    let mut segment_id = 0;
    
    while current_start_s < duration_s {
        let current_end_s = (current_start_s + config.segment_duration_s).min(duration_s);
        
        // Check if last segment would be too short
        let remaining = duration_s - current_end_s;
        let is_last = remaining < config.min_segment_duration_s;
        
        let actual_end_s = if is_last {
            duration_s  // Extend last segment to the end
        } else {
            current_end_s
        };
        
        // Extract samples for this segment
        let start_sample = (current_start_s * samples_per_second) as usize;
        let end_sample = (actual_end_s * samples_per_second) as usize;
        let end_sample = end_sample.min(audio.samples.len());
        
        let segment_samples = audio.samples[start_sample..end_sample].to_vec();
        
        segments.push(AudioSegment {
            segment_id,
            start_time_s: current_start_s,
            end_time_s: actual_end_s,
            samples: segment_samples,
            sample_rate: audio.sample_rate,
        });
        
        if is_last {
            break;
        }
        
        current_start_s += step_duration_s;
        segment_id += 1;
    }
    
    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_no_segmentation_for_short_audio() {
        let audio = AudioData {
            samples: vec![0.0; 16000 * 20], // 20 seconds
            sample_rate: 16000,
            channels: 1,
            duration_ms: 20000,
        };
        
        let config = SegmentationConfig::default();
        assert!(!should_segment(&audio, &config));
        
        let segments = segment_audio(&audio, &config);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_id, 0);
    }
    
    #[test]
    fn test_segmentation_for_long_audio() {
        let audio = AudioData {
            samples: vec![0.0; 16000 * 60], // 60 seconds
            sample_rate: 16000,
            channels: 1,
            duration_ms: 60000,
        };
        
        let config = SegmentationConfig::default();
        assert!(should_segment(&audio, &config));
        
        let segments = segment_audio(&audio, &config);
        
        // 60s with 25s segments and 20s step = 3 segments
        // Seg 0: 0-25, Seg 1: 20-45, Seg 2: 40-60
        assert_eq!(segments.len(), 3);
        
        // Check overlap
        assert!((segments[0].end_time_s - segments[1].start_time_s - 5.0).abs() < 0.1);
    }
}
