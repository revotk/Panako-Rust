//! Panako Database Layer
//!
//! PostgreSQL integration for fingerprint storage and retrieval

pub mod connection;
pub mod models;
pub mod operations;

// Re-export commonly used types
pub use connection::{create_pool, test_connection};
pub use models::{
    Fingerprint, FingerprintMetadata, FingerprintQuery, FingerprintSummary,
    NewFingerprint, NewFingerprintMetadata, NewSegment, NewSegmentationConfig,
    Segment, SegmentationConfig,
};
pub use operations::{
    delete_metadata, get_all_metadata, get_fingerprint_summaries,
    get_fingerprints_by_hash, get_fingerprints_by_metadata, get_metadata_by_filename,
    get_metadata_by_id, get_segments_by_metadata, insert_fingerprints_batch,
    insert_metadata, insert_segment, insert_segmentation_config, query_fingerprints,
};
