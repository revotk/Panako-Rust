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
