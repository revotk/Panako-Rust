//! Event point extraction using 2D max filtering
//!
//! Implements the Panako event point extraction algorithm.

use crate::config::PanakoConfig;
use crate::transform::Spectrogram;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// An event point represents a local maximum in the spectrogram
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EventPoint {
    /// Time index (frame number)
    pub t: i32,
    /// Frequency bin index
    pub f: i16,
    /// Magnitude value
    pub m: f32,
}

impl EventPoint {
    pub fn new(t: i32, f: i16, m: f32) -> Self {
        Self { t, f, m }
    }
}

/// Event point extractor
pub struct EventPointExtractor {
    freq_filter_size: usize,
    time_filter_size: usize,
}

impl EventPointExtractor {
    pub fn new(config: &PanakoConfig) -> Self {
        Self {
            freq_filter_size: config.freq_max_filter_size,
            time_filter_size: config.time_max_filter_size,
        }
    }
    
    /// Extract event points from spectrogram
    pub fn extract(&self, spectrogram: &Spectrogram) -> Result<Vec<EventPoint>> {
        // Apply 2D max filtering
        let max_filtered = self.apply_2d_max_filter(spectrogram);
        
        // Find local maxima
        let event_points = self.find_local_maxima(spectrogram, &max_filtered);
        
        Ok(event_points)
    }
    
    /// Apply 2D max filter (frequency then time)
    fn apply_2d_max_filter(&self, spectrogram: &Spectrogram) -> Vec<Vec<f32>> {
        let num_frames = spectrogram.num_frames;
        let num_bins = spectrogram.num_bins;
        
        // First, filter in frequency dimension
        let mut freq_filtered = vec![vec![0.0; num_bins]; num_frames];
        
        for t in 0..num_frames {
            for f in 0..num_bins {
                let f_start = f.saturating_sub(self.freq_filter_size / 2);
                let f_end = (f + self.freq_filter_size / 2 + 1).min(num_bins);
                
                let max_val = (f_start..f_end)
                    .map(|fi| spectrogram.magnitudes[t][fi])
                    .fold(f32::NEG_INFINITY, f32::max);
                
                freq_filtered[t][f] = max_val;
            }
        }
        
        // Then, filter in time dimension
        let mut time_filtered = vec![vec![0.0; num_bins]; num_frames];
        
        for t in 0..num_frames {
            let t_start = t.saturating_sub(self.time_filter_size / 2);
            let t_end = (t + self.time_filter_size / 2 + 1).min(num_frames);
            
            for f in 0..num_bins {
                let max_val = (t_start..t_end)
                    .map(|ti| freq_filtered[ti][f])
                    .fold(f32::NEG_INFINITY, f32::max);
                
                time_filtered[t][f] = max_val;
            }
        }
        
        time_filtered
    }
    
    /// Find local maxima by comparing original with max-filtered
    fn find_local_maxima(
        &self,
        spectrogram: &Spectrogram,
        max_filtered: &[Vec<f32>],
    ) -> Vec<EventPoint> {
        let mut event_points = Vec::new();
        
        for t in 0..spectrogram.num_frames {
            for f in 0..spectrogram.num_bins {
                let original = spectrogram.magnitudes[t][f];
                let filtered = max_filtered[t][f];
                
                // If original equals max-filtered, it's a local maximum
                if original > 0.0 && (original - filtered).abs() < 1e-6 {
                    event_points.push(EventPoint::new(
                        t as i32,
                        f as i16,
                        original,
                    ));
                }
            }
        }
        
        event_points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_point_creation() {
        let ep = EventPoint::new(100, 50, 0.8);
        assert_eq!(ep.t, 100);
        assert_eq!(ep.f, 50);
        assert!((ep.m - 0.8).abs() < 1e-6);
    }
}
