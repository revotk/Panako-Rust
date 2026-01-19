//! Audio decoding and resampling
//!
//! Supports WAV, MP3, FLAC, OGG, and video formats (MP4, AVI, TS, etc.) using pure Rust decoders.

mod decoder;
mod resample;
mod video;
mod ts;

pub use decoder::{decode_audio, AudioData};
pub use resample::resample_to_target;
pub use video::extract_audio_from_video;
pub use ts::extract_audio_from_ts;

use std::path::Path;

/// Supported audio and video formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    // Pure audio formats
    Wav,
    Mp3,
    Flac,
    Ogg,
    
    // Video container formats (extract audio)
    Mp4,
    Mkv,
    Avi,
    MpegTs,
    Mov,
    Webm,
    
    Unknown,
}

impl AudioFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            // Audio formats
            Some("wav") | Some("wave") => AudioFormat::Wav,
            Some("mp3") => AudioFormat::Mp3,
            Some("flac") => AudioFormat::Flac,
            Some("ogg") => AudioFormat::Ogg,
            
            // Video formats
            Some("mp4") | Some("m4a") | Some("m4v") => AudioFormat::Mp4,
            Some("mkv") => AudioFormat::Mkv,
            Some("avi") => AudioFormat::Avi,
            Some("ts") | Some("mts") | Some("m2ts") => AudioFormat::MpegTs,
            Some("mov") => AudioFormat::Mov,
            Some("webm") => AudioFormat::Webm,
            
            _ => AudioFormat::Unknown,
        }
    }
    
    /// Check if format is a video container
    pub fn is_video_container(&self) -> bool {
        matches!(self, 
            AudioFormat::Mp4 | 
            AudioFormat::Mkv | 
            AudioFormat::Avi | 
            AudioFormat::MpegTs | 
            AudioFormat::Mov | 
            AudioFormat::Webm
        )
    }
}
