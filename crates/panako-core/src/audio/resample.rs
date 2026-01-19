//! Audio resampling using simple linear interpolation
//! TODO: Replace with proper rubato implementation once API is clarified

use anyhow::Result;

/// Resample audio to target sample rate using linear interpolation
pub fn resample_to_target(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }
    
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);
    
    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = src_pos - src_idx as f64;
        
        if src_idx + 1 < samples.len() {
            // Linear interpolation
            let val = samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32;
            output.push(val);
        } else if src_idx < samples.len() {
            output.push(samples[src_idx]);
        }
    }
    
    Ok(output)
}

