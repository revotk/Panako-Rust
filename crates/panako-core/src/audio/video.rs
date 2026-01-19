//! Video demuxing and audio extraction using Symphonia

use super::AudioData;
use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Extract audio from video file using Symphonia
pub fn extract_audio_from_video(path: &Path) -> Result<AudioData> {
    // Open the media file
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open video file: {}", path.display()))?;
    
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    
    // Create a hint to help the format registry guess the format
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    
    // Configure format options for better error tolerance
    let format_opts = FormatOptions {
        enable_gapless: true,
        prebuild_seek_index: false,
        seek_index_fill_rate: 20,
    };
    
    // Probe the media source
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &MetadataOptions::default())
        .with_context(|| format!("Failed to probe video file: {}", path.display()))?;
    
    let mut format = probed.format;
    
    // Find the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("No audio track found in video file"))?;
    
    let track_id = track.id;
    let codec_params = &track.codec_params;
    
    // Get sample rate and channels
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(2) as u16;
    
    // Create a decoder for the track
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .with_context(|| "Failed to create audio decoder")?;
    
    // Decode all packets
    let mut samples = Vec::new();
    
    loop {
        // Get the next packet
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e)) 
                if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(anyhow::anyhow!("Error reading packet: {}", e)),
        };
        
        // Skip packets that don't belong to our audio track
        if packet.track_id() != track_id {
            continue;
        }
        
        // Decode the packet
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(_) => {
                // Skip corrupted packets and continue
                continue;
            }
        };
        
        // Convert decoded audio to f32 samples
        match decoded {
            AudioBufferRef::F32(buf) => {
                // Interleave channels
                for frame_idx in 0..buf.frames() {
                    for ch in 0..buf.spec().channels.count() {
                        samples.push(buf.chan(ch)[frame_idx]);
                    }
                }
            }
            AudioBufferRef::F64(buf) => {
                for frame_idx in 0..buf.frames() {
                    for ch in 0..buf.spec().channels.count() {
                        samples.push(buf.chan(ch)[frame_idx] as f32);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for frame_idx in 0..buf.frames() {
                    for ch in 0..buf.spec().channels.count() {
                        samples.push(buf.chan(ch)[frame_idx] as f32 / i32::MAX as f32);
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                for frame_idx in 0..buf.frames() {
                    for ch in 0..buf.spec().channels.count() {
                        samples.push(buf.chan(ch)[frame_idx] as f32 / i16::MAX as f32);
                    }
                }
            }
            AudioBufferRef::U8(buf) => {
                for frame_idx in 0..buf.frames() {
                    for ch in 0..buf.spec().channels.count() {
                        samples.push((buf.chan(ch)[frame_idx] as f32 - 128.0) / 128.0);
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported audio buffer format"));
            }
        }
    }
    
    let duration_ms = (samples.len() as f64 / (sample_rate * channels as u32) as f64 * 1000.0) as u32;
    
    Ok(AudioData {
        samples,
        sample_rate,
        channels,
        duration_ms,
    })
}
