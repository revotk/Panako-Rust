//! Spectral transform using Constant-Q approximation
//!
//! Implements a Gabor-like transform using FFT + constant-Q filterbank
//! to match the Java JGaborator behavior.

use crate::config::PanakoConfig;
use anyhow::Result;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;

/// Spectrogram representation
#[derive(Debug, Clone)]
pub struct Spectrogram {
    /// Magnitude values [time_frame][frequency_bin]
    pub magnitudes: Vec<Vec<f32>>,
    /// Number of time frames
    pub num_frames: usize,
    /// Number of frequency bins
    pub num_bins: usize,
}

/// Compute spectral transform (Constant-Q approximation)
pub fn compute_transform(samples: &[f32], config: &PanakoConfig) -> Result<Spectrogram> {
    let hop_size = config.time_resolution;
    let fft_size = config.audio_block_size;
    
    // Calculate number of frames
    let num_frames = (samples.len() / hop_size).saturating_sub(1);
    
    // Calculate frequency bins for constant-Q
    let num_bins = calculate_num_bins(config);
    
    // Initialize FFT planner
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    
    // Hann window
    let window = create_hann_window(fft_size);
    
    // Process each frame
    let mut magnitudes = Vec::with_capacity(num_frames);
    
    for frame_idx in 0..num_frames {
        let start = frame_idx * hop_size;
        let end = (start + fft_size).min(samples.len());
        
        // Extract and window frame
        let mut frame: Vec<Complex<f32>> = samples[start..end]
            .iter()
            .enumerate()
            .map(|(i, &s)| Complex::new(s * window[i], 0.0))
            .collect();
        
        // Pad if needed
        frame.resize(fft_size, Complex::new(0.0, 0.0));
        
        // Compute FFT
        fft.process(&mut frame);
        
        // Map FFT bins to constant-Q bins
        let cq_magnitudes = map_to_constant_q(&frame, config, num_bins);
        magnitudes.push(cq_magnitudes);
    }
    
    Ok(Spectrogram {
        magnitudes,
        num_frames,
        num_bins,
    })
}

/// Calculate number of constant-Q bins
fn calculate_num_bins(config: &PanakoConfig) -> usize {
    let octaves = (config.max_freq / config.min_freq).log2();
    (octaves * config.bands_per_octave as f32).ceil() as usize
}

/// Create Hann window
fn create_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let x = i as f32 / (size - 1) as f32;
            0.5 * (1.0 - (2.0 * PI * x).cos())
        })
        .collect()
}

/// Map FFT bins to constant-Q bins
fn map_to_constant_q(
    fft_output: &[Complex<f32>],
    config: &PanakoConfig,
    num_bins: usize,
) -> Vec<f32> {
    let sample_rate = config.sample_rate as f32;
    let fft_size = fft_output.len();
    
    let mut cq_bins = vec![0.0; num_bins];
    
    for bin_idx in 0..num_bins {
        // Calculate center frequency for this constant-Q bin
        let freq = config.min_freq * 2.0_f32.powf(bin_idx as f32 / config.bands_per_octave as f32);
        
        // Map to FFT bin
        let fft_bin = (freq * fft_size as f32 / sample_rate) as usize;
        
        if fft_bin < fft_size / 2 {
            // Calculate magnitude
            let magnitude = fft_output[fft_bin].norm();
            cq_bins[bin_idx] = magnitude;
        }
    }
    
    cq_bins
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hann_window() {
        let window = create_hann_window(512);
        assert_eq!(window.len(), 512);
        assert!((window[0] - 0.0).abs() < 0.001);
        assert!((window[256] - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_num_bins_calculation() {
        let config = PanakoConfig::default();
        let num_bins = calculate_num_bins(&config);
        // 6 octaves * 85 bands = 510 bins
        assert!(num_bins >= 500 && num_bins <= 520);
    }
}
