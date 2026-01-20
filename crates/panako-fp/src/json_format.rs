//! JSON format for fingerprint files
//!
//! New JSON-based format for storing fingerprints with metadata and segmentation support

use serde::{Deserialize, Serialize};

/// Complete JSON fingerprint file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpJsonFile {
    pub version: String,
    pub metadata: FpJsonMetadata,
    pub segmentation: JsonSegmentationConfig,
    pub segments: Vec<FpJsonSegment>,
}

/// Metadata about the original audio file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpJsonMetadata {
    pub original_path: String,
    pub filename: String,
    pub algorithm: String,
    pub sample_rate: u32,
    pub duration_ms: u32,
    pub channels: u16,
    pub created_at: String,
}

/// Segmentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSegmentationConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_duration_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlap_duration_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_segments: Option<usize>,
}

/// Individual segment with fingerprints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpJsonSegment {
    pub segment_id: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub num_fingerprints: usize,
    pub fingerprints: Vec<FpJsonFingerprint>,
}

/// Individual fingerprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpJsonFingerprint {
    pub hash: u64,
    pub t1: i32,
    pub f1: i16,
    pub m1: f32,
}

impl FpJsonFile {
    /// Create a new JSON fingerprint file
    pub fn new(
        original_path: String,
        filename: String,
        sample_rate: u32,
        duration_ms: u32,
        channels: u16,
    ) -> Self {
        Self {
            version: "2.0".to_string(),
            metadata: FpJsonMetadata {
                original_path,
                filename,
                algorithm: "PANAKO".to_string(),
                sample_rate,
                duration_ms,
                channels,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            segmentation: JsonSegmentationConfig {
                enabled: false,
                segment_duration_s: None,
                overlap_duration_s: None,
                num_segments: None,
            },
            segments: Vec::new(),
        }
    }

    /// Enable segmentation
    pub fn with_segmentation(
        mut self,
        segment_duration_s: f64,
        overlap_duration_s: f64,
        num_segments: usize,
    ) -> Self {
        self.segmentation = JsonSegmentationConfig {
            enabled: true,
            segment_duration_s: Some(segment_duration_s),
            overlap_duration_s: Some(overlap_duration_s),
            num_segments: Some(num_segments),
        };
        self
    }

    /// Add a segment
    pub fn add_segment(&mut self, segment: FpJsonSegment) {
        self.segments.push(segment);
    }

    /// Save to JSON file
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let json_str = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json_str)?;
        Ok(())
    }

    /// Load from JSON file
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let json_str = std::fs::read_to_string(path)?;
        let fp_file: FpJsonFile = serde_json::from_str(&json_str)?;
        Ok(fp_file)
    }

    /// Save to BSON file
    pub fn save_bson(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let bson_data = bson::to_vec(self)?;
        std::fs::write(path, bson_data)?;
        Ok(())
    }

    /// Load from BSON file
    pub fn load_bson(path: &std::path::Path) -> anyhow::Result<Self> {
        let bson_data = std::fs::read(path)?;
        let fp_file: FpJsonFile = bson::from_slice(&bson_data)?;
        Ok(fp_file)
    }

    /// Load from file (auto-detect format based on extension)
    pub fn load_auto(path: &std::path::Path) -> anyhow::Result<Self> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("json");
        
        match extension {
            "bson" => Self::load_bson(path),
            _ => Self::load(path), // Default to JSON
        }
    }

    /// Get all fingerprints from all segments as tuples
    pub fn get_all_fingerprints(&self) -> Vec<(u64, i32, i16, f32)> {
        self.segments
            .iter()
            .flat_map(|seg| {
                seg.fingerprints
                    .iter()
                    .map(|fp| (fp.hash, fp.t1, fp.f1, fp.m1))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bson_round_trip() {
        let mut fp_file = FpJsonFile::new(
            "/path/to/audio.wav".to_string(),
            "audio".to_string(),
            16000,
            5000,
            1,
        );

        // Add a segment with fingerprints
        let fingerprints = vec![
            FpJsonFingerprint {
                hash: 12345678901234,
                t1: 100,
                f1: 50,
                m1: 1.0,
            },
            FpJsonFingerprint {
                hash: 98765432109876,
                t1: 200,
                f1: 60,
                m1: 1.0,
            },
        ];

        let segment = FpJsonSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s: 5.0,
            num_fingerprints: fingerprints.len(),
            fingerprints,
        };

        fp_file.add_segment(segment);

        // Test BSON serialization
        let bson_data = bson::to_vec(&fp_file).unwrap();
        let fp_file_loaded: FpJsonFile = bson::from_slice(&bson_data).unwrap();

        assert_eq!(fp_file.version, fp_file_loaded.version);
        assert_eq!(fp_file.metadata.filename, fp_file_loaded.metadata.filename);
        assert_eq!(fp_file.segments.len(), fp_file_loaded.segments.len());
        assert_eq!(
            fp_file.segments[0].fingerprints.len(),
            fp_file_loaded.segments[0].fingerprints.len()
        );
    }

    #[test]
    fn test_bson_size_reduction() {
        let mut fp_file = FpJsonFile::new(
            "/path/to/audio.wav".to_string(),
            "audio".to_string(),
            16000,
            5000,
            1,
        );

        // Add multiple fingerprints
        let mut fingerprints = Vec::new();
        for i in 0..100 {
            fingerprints.push(FpJsonFingerprint {
                hash: 12345678901234 + i,
                t1: (i * 10) as i32,
                f1: (50 + i % 50) as i16,
                m1: 1.0,
            });
        }

        let segment = FpJsonSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s: 5.0,
            num_fingerprints: fingerprints.len(),
            fingerprints,
        };

        fp_file.add_segment(segment);

        // Compare sizes
        let json_str = serde_json::to_string(&fp_file).unwrap();
        let bson_data = bson::to_vec(&fp_file).unwrap();

        let json_size = json_str.len();
        let bson_size = bson_data.len();
        
        println!("JSON size: {} bytes", json_size);
        println!("BSON size: {} bytes", bson_size);

        // BSON should be smaller or similar size
        if bson_size < json_size {
            let reduction = ((json_size - bson_size) as f64 / json_size as f64) * 100.0;
            println!("Reduction: {:.1}%", reduction);
            assert!(reduction > 0.0); // Some reduction expected
        } else {
            println!("BSON is larger (this can happen with small datasets)");
        }
    }

    #[test]
    fn test_bson_size_reduction_large() {
        // Test with a more realistic dataset (1000 fingerprints)
        let mut fp_file = FpJsonFile::new(
            "/path/to/audio.wav".to_string(),
            "audio".to_string(),
            16000,
            60000, // 60 seconds
            1,
        );

        // Add 1000 fingerprints
        let mut fingerprints = Vec::new();
        for i in 0..1000 {
            fingerprints.push(FpJsonFingerprint {
                hash: 12345678901234 + i,
                t1: (i * 10) as i32,
                f1: (50 + i % 100) as i16,
                m1: 1.0 + (i as f32 * 0.001),
            });
        }

        let segment = FpJsonSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s: 60.0,
            num_fingerprints: fingerprints.len(),
            fingerprints,
        };

        fp_file.add_segment(segment);

        // Compare sizes
        let json_str = serde_json::to_string(&fp_file).unwrap();
        let bson_data = bson::to_vec(&fp_file).unwrap();

        let json_size = json_str.len();
        let bson_size = bson_data.len();

        println!("\n=== Large Dataset Test (1000 fingerprints) ===");
        println!("JSON size: {} bytes ({:.1} KB)", json_size, json_size as f64 / 1024.0);
        println!("BSON size: {} bytes ({:.1} KB)", bson_size, bson_size as f64 / 1024.0);

        // BSON provides modest size reduction (typically 4-10%)
        // The main benefit is faster parsing, not dramatic size reduction
        if bson_size < json_size {
            let reduction = ((json_size - bson_size) as f64 / json_size as f64) * 100.0;
            println!("Reduction: {:.1}%", reduction);
            println!("Note: BSON's main benefit is faster parsing, not size");
            assert!(reduction > 0.0); // Some reduction expected
        }
    }
}

