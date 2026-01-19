//! .fp file format structures

use serde::{Deserialize, Serialize};

/// Magic bytes for .fp files: "FPAN"
pub const MAGIC: [u8; 4] = [0x46, 0x50, 0x41, 0x4E];

/// Current format version
pub const VERSION: u16 = 1;

/// File header (64 bytes fixed size)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpHeader {
    /// Magic bytes: "FPAN"
    pub magic: [u8; 4],
    /// Format version
    pub version: u16,
    /// Flags (bit 0: compressed)
    pub flags: u16,
    /// Size of metadata section
    pub metadata_size: u64,
    /// Size of payload (uncompressed)
    pub payload_size: u64,
    /// Compressed payload size (0 if uncompressed)
    pub payload_size_compressed: u64,
    /// Number of fingerprints
    pub num_fingerprints: u32,
    /// Sample rate (Hz)
    pub sample_rate: u32,
    /// Duration (milliseconds)
    pub duration_ms: u32,
    /// Number of channels
    pub channels: u16,
    /// Reserved
    pub reserved1: u16,
    /// CRC64 checksum
    pub checksum: u64,
    /// Reserved
    pub reserved2: u64,
}

impl FpHeader {
    pub fn new(
        metadata_size: u64,
        payload_size: u64,
        num_fingerprints: u32,
        sample_rate: u32,
        duration_ms: u32,
        channels: u16,
    ) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            flags: 0,
            metadata_size,
            payload_size,
            payload_size_compressed: 0,
            num_fingerprints,
            sample_rate,
            duration_ms,
            channels,
            reserved1: 0,
            checksum: 0,
            reserved2: 0,
        }
    }
    
    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x1) != 0
    }
    
    pub fn set_compressed(&mut self, compressed: bool) {
        if compressed {
            self.flags |= 0x1;
        } else {
            self.flags &= !0x1;
        }
    }
}

/// Segmentation information for monitor mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationInfo {
    pub num_segments: usize,
    pub segment_duration_ms: u32,
    pub overlap_duration_ms: u32,
    pub segments: Vec<SegmentMetadata>,
}

/// Metadata for individual segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMetadata {
    pub segment_id: usize,
    pub start_time_ms: u32,
    pub end_time_ms: u32,
    pub num_fingerprints: u32,
    pub fingerprint_offset: u32,
}

/// Metadata section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpMetadata {
    /// Algorithm ID (e.g., "PANAKO")
    pub algorithm_id: String,
    /// Algorithm parameters (JSON)
    pub algorithm_params: String,
    /// Original filename
    pub original_filename: String,
    /// Segmentation info (None if not segmented)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segmentation: Option<SegmentationInfo>,
}

/// Complete .fp file structure
#[derive(Debug, Clone)]
pub struct FpFile {
    pub header: FpHeader,
    pub metadata: FpMetadata,
    /// Fingerprint data: (hash, t1, f1, m1)
    pub fingerprints: Vec<(u64, i32, i16, f32)>,
}
