//! Configuration parameters for the Panako algorithm
//!
//! These values match the Java reference implementation defaults.

use serde::{Deserialize, Serialize};

/// Algorithm configuration matching Java Panako defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanakoConfig {
    // Audio processing
    pub sample_rate: u32,
    pub audio_block_size: usize,
    pub audio_block_overlap: usize,
    
    // Spectral transform (Gabor/Constant-Q)
    pub min_freq: f32,
    pub max_freq: f32,
    pub bands_per_octave: u32,
    pub ref_freq: f32,
    pub time_resolution: usize,
    
    // Event point extraction
    pub freq_max_filter_size: usize,
    pub time_max_filter_size: usize,
    
    // Fingerprint generation
    pub fp_min_freq_dist: i16,
    pub fp_max_freq_dist: i16,
    pub fp_min_time_dist: i32,
    pub fp_max_time_dist: i32,
    
    // Matching parameters
    pub query_range: i32,
    pub min_hits_unfiltered: usize,
    pub min_hits_filtered: usize,
    pub min_time_factor: f64,
    pub max_time_factor: f64,
    pub min_freq_factor: f64,
    pub max_freq_factor: f64,
    pub min_sec_with_match: f64,
    pub min_match_duration: f64,
}

impl Default for PanakoConfig {
    fn default() -> Self {
        Self {
            // Audio processing - matching Java defaults
            sample_rate: 16000,
            audio_block_size: 8192,
            audio_block_overlap: 0,
            
            // Spectral transform
            min_freq: 110.0,
            max_freq: 7040.0,
            bands_per_octave: 85,
            ref_freq: 440.0,
            time_resolution: 128,
            
            // Event point extraction
            freq_max_filter_size: 103,
            time_max_filter_size: 25,
            
            // Fingerprint generation
            fp_min_freq_dist: 1,
            fp_max_freq_dist: 128,
            fp_min_time_dist: 2,
            fp_max_time_dist: 33,
            
            // Matching parameters
            query_range: 2,
            min_hits_unfiltered: 10,
            min_hits_filtered: 5,
            min_time_factor: 0.8,
            max_time_factor: 1.2,
            min_freq_factor: 0.8,
            max_freq_factor: 1.2,
            min_sec_with_match: 0.2,
            min_match_duration: 3.0,
        }
    }
}

impl PanakoConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.sample_rate == 0 {
            anyhow::bail!("Sample rate must be > 0");
        }
        if self.min_freq >= self.max_freq {
            anyhow::bail!("min_freq must be < max_freq");
        }
        if self.bands_per_octave == 0 {
            anyhow::bail!("bands_per_octave must be > 0");
        }
        Ok(())
    }
}
