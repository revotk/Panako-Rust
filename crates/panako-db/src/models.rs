use serde::{Deserialize, Serialize};
use bson::Bson;

/// Represents metadata for a fingerprint file stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintMetadata {
    pub id: i32,
    pub original_path: String,
    pub filename: String,
    pub sample_rate: i32,
    pub duration_ms: i32,
    pub channels: i16,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Represents segmentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationConfig {
    pub id: i32,
    pub metadata_id: i32,
    pub enabled: bool,
    pub segment_duration_ms: Option<i32>,
    pub overlap_ms: Option<i32>,
}

/// Represents a single segment within a fingerprint file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: i32,
    pub metadata_id: i32,
    pub segment_index: i32,
    pub start_ms: i32,
    pub end_ms: i32,
}

/// Represents a fingerprint stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub id: i64,
    pub metadata_id: i32,
    pub segment_id: Option<i32>,
    pub hash: i64,
    pub t1: i32,
    pub f1: i16,
    pub m1: f32,
}

/// Input structure for creating new fingerprint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFingerprintMetadata {
    pub original_path: String,
    pub filename: String,
    pub sample_rate: i32,
    pub duration_ms: i32,
    pub channels: i16,
}

/// Input structure for creating new segmentation config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSegmentationConfig {
    pub metadata_id: i32,
    pub enabled: bool,
    pub segment_duration_ms: Option<i32>,
    pub overlap_ms: Option<i32>,
}

/// Input structure for creating new segments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSegment {
    pub metadata_id: i32,
    pub segment_index: i32,
    pub start_ms: i32,
    pub end_ms: i32,
}

/// Input structure for creating new fingerprints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFingerprint {
    pub metadata_id: i32,
    pub segment_id: Option<i32>,
    pub hash: i64,
    pub t1: i32,
    pub f1: i16,
    pub m1: f32,
}

/// Query criteria for retrieving fingerprints
#[derive(Debug, Clone, Default)]
pub struct FingerprintQuery {
    pub metadata_id: Option<i32>,
    pub segment_id: Option<i32>,
    pub hash: Option<i64>,
    pub limit: Option<i64>,
}

/// Summary information about a fingerprint file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintSummary {
    pub metadata_id: i32,
    pub filename: String,
    pub duration_ms: i32,
    pub total_segments: i64,
    pub total_fingerprints: i64,
}

impl From<Bson> for Fingerprint {
    fn from(bson: Bson) -> Self {
        // This is a helper for converting JSONB data from PostgreSQL
        // The actual conversion will be handled by serde_json since PostgreSQL
        // returns JSONB as JSON text
        if let Bson::Document(doc) = bson {
            Fingerprint {
                id: doc.get_i64("id").unwrap_or(0),
                metadata_id: doc.get_i32("metadata_id").unwrap_or(0),
                segment_id: doc.get_i32("segment_id").ok(),
                hash: doc.get_i64("hash").unwrap_or(0),
                t1: doc.get_i32("t1").unwrap_or(0),
                f1: doc.get_i32("f1").unwrap_or(0) as i16,
                m1: doc.get_f64("m1").unwrap_or(0.0) as f32,
            }
        } else {
            panic!("Expected BSON document")
        }
    }
}
