//! Fingerprint generation and hashing
//!
//! Implements the Panako fingerprint algorithm that connects 3 event points
//! and computes a 64-bit hash.

use crate::config::PanakoConfig;
use crate::eventpoint::EventPoint;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A fingerprint connects three event points
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// 64-bit hash of the fingerprint
    pub hash: u64,
    /// Time index of first event point
    pub t1: i32,
    /// Frequency bin of first event point
    pub f1: i16,
    /// Magnitude of first event point (optional, for debugging)
    pub m1: f32,
    
    // Store all event points for hash computation
    pub t2: i32,
    pub f2: i16,
    pub m2: f32,
    pub t3: i32,
    pub f3: i16,
    pub m3: f32,
}

impl Fingerprint {
    /// Create fingerprint from three event points
    pub fn new(e1: &EventPoint, e2: &EventPoint, e3: &EventPoint) -> Self {
        let mut fp = Self {
            hash: 0,
            t1: e1.t,
            f1: e1.f,
            m1: e1.m,
            t2: e2.t,
            f2: e2.f,
            m2: e2.m,
            t3: e3.t,
            f3: e3.f,
            m3: e3.m,
        };
        
        // Compute hash
        fp.hash = fp.compute_hash();
        fp
    }
    
    /// Compute 64-bit hash matching Java implementation
    /// This is the exact algorithm from PanakoFingerprint.java
    fn compute_hash(&self) -> u64 {
        let f1 = self.f1 as i32;
        let f2 = self.f2 as i32;
        let f3 = self.f3 as i32;
        let m1 = self.m1;
        let m2 = self.m2;
        let m3 = self.m3;
        let t1 = self.t1;
        let t2 = self.t2;
        let t3 = self.t3;
        
        // Comparison bits
        let f1_larger_than_f2 = if f1 > f2 { 1u64 } else { 0u64 };
        let f2_larger_than_f3 = if f2 > f3 { 1u64 } else { 0u64 };
        let f3_larger_than_f1 = if f3 > f1 { 1u64 } else { 0u64 };
        
        let m1_larger_than_m2 = if m1 > m2 { 1u64 } else { 0u64 };
        let m2_larger_than_m3 = if m2 > m3 { 1u64 } else { 0u64 };
        let m3_larger_than_m1 = if m3 > m1 { 1u64 } else { 0u64 };
        
        let dt1t2_larger_than_t3t2 = if (t2 - t1) > (t3 - t2) { 1u64 } else { 0u64 };
        let df1f2_larger_than_f3f2 = if (f2 - f1).abs() > (f3 - f2).abs() { 1u64 } else { 0u64 };
        
        // Frequency range (9 bits -> 8 bits)
        let f1_range = ((f1 >> 5) & 0xFF) as u64;
        
        // Frequency differences (7 bits -> 6 bits)
        let df2f1 = (((f2 - f1).abs() >> 2) & 0x3F) as u64;
        let df3f2 = (((f3 - f2).abs() >> 2) & 0x3F) as u64;
        
        // Time ratio (6 bits)
        let ratio_t = ((t2 - t1) as f32 / (t3 - t1) as f32 * 64.0) as u64 & 0x3F;
        
        // Combine into 64-bit hash
        let hash = 
            (ratio_t                    & 0x3F)  << 0  |
            (f1_larger_than_f2          & 0x1)   << 6  |
            (f2_larger_than_f3          & 0x1)   << 7  |
            (f3_larger_than_f1          & 0x1)   << 8  |
            (m1_larger_than_m2          & 0x1)   << 9  |
            (m2_larger_than_m3          & 0x1)   << 10 |
            (m3_larger_than_m1          & 0x1)   << 11 |
            (dt1t2_larger_than_t3t2     & 0x1)   << 12 |
            (df1f2_larger_than_f3f2     & 0x1)   << 13 |
            (f1_range                   & 0xFF)  << 14 |
            (df2f1                      & 0x3F)  << 22 |
            (df3f2                      & 0x3F)  << 28;
        
        hash
    }
}

/// Fingerprint generator
pub struct FingerprintGenerator {
    min_freq_dist: i16,
    max_freq_dist: i16,
    min_time_dist: i32,
    max_time_dist: i32,
}

impl FingerprintGenerator {
    pub fn new(config: &PanakoConfig) -> Self {
        Self {
            min_freq_dist: config.fp_min_freq_dist,
            max_freq_dist: config.fp_max_freq_dist,
            min_time_dist: config.fp_min_time_dist,
            max_time_dist: config.fp_max_time_dist,
        }
    }
    
    /// Generate fingerprints from event points
    pub fn generate(&self, event_points: &[EventPoint]) -> Result<Vec<Fingerprint>> {
        let mut fingerprints = Vec::new();
        
        // For each event point, find valid pairs to form fingerprints
        for i in 0..event_points.len() {
            let e1 = &event_points[i];
            
            // Find second event point
            for j in (i + 1)..event_points.len() {
                let e2 = &event_points[j];
                
                // Check constraints for e1-e2
                let dt12 = e2.t - e1.t;
                let df12 = (e2.f - e1.f).abs();
                
                if dt12 < self.min_time_dist || dt12 > self.max_time_dist {
                    continue;
                }
                if df12 < self.min_freq_dist || df12 > self.max_freq_dist {
                    continue;
                }
                
                // Find third event point
                for k in (j + 1)..event_points.len() {
                    let e3 = &event_points[k];
                    
                    // Check constraints for e2-e3
                    let dt23 = e3.t - e2.t;
                    let df23 = (e3.f - e2.f).abs();
                    
                    if dt23 < self.min_time_dist || dt23 > self.max_time_dist {
                        continue;
                    }
                    if df23 < self.min_freq_dist || df23 > self.max_freq_dist {
                        continue;
                    }
                    
                    // Create fingerprint
                    fingerprints.push(Fingerprint::new(e1, e2, e3));
                }
            }
        }
        
        // Sort by t1 for deterministic output
        fingerprints.sort_by_key(|fp| fp.t1);
        
        Ok(fingerprints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fingerprint_hash() {
        let e1 = EventPoint::new(0, 100, 0.5);
        let e2 = EventPoint::new(10, 120, 0.7);
        let e3 = EventPoint::new(20, 110, 0.6);
        
        let fp = Fingerprint::new(&e1, &e2, &e3);
        
        // Hash should be non-zero
        assert_ne!(fp.hash, 0);
        
        // Same event points should produce same hash
        let fp2 = Fingerprint::new(&e1, &e2, &e3);
        assert_eq!(fp.hash, fp2.hash);
    }
}
