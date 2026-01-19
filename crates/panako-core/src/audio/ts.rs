//! MPEG-TS demuxing and audio extraction

use super::AudioData;
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

/// Extract audio from MPEG-TS file using FFmpeg pipe
/// 
/// This function spawns FFmpeg as a subprocess and reads the audio
/// data directly from stdout as WAV format, avoiding temporary files.
pub fn extract_audio_from_ts(path: &Path) -> Result<AudioData> {
    // Check if FFmpeg is available
    let ffmpeg_check = Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    
    if ffmpeg_check.is_err() {
        anyhow::bail!(
            "FFmpeg not found. MPEG-TS files require FFmpeg to extract audio.\n\
            \n\
            Please install FFmpeg:\n\
            - Windows: Download from https://ffmpeg.org/download.html\n\
            - Linux: sudo apt install ffmpeg\n\
            - macOS: brew install ffmpeg\n\
            \n\
            Alternatively, convert the file manually:\n\
            ffmpeg -i {} -vn -acodec pcm_s16le -ar 16000 -ac 1 output.wav",
            path.display()
        );
    }
    
    // Spawn FFmpeg to extract audio as raw PCM to stdout
    let mut child = Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .arg("-vn")                    // No video
        .arg("-acodec")
        .arg("pcm_s16le")              // PCM 16-bit little-endian
        .arg("-ar")
        .arg("16000")                  // 16kHz sample rate
        .arg("-ac")
        .arg("1")                      // Mono
        .arg("-f")
        .arg("s16le")                  // Raw PCM format (no WAV header)
        .arg("pipe:1")                 // Output to stdout
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())         // Suppress FFmpeg output
        .spawn()
        .with_context(|| "Failed to spawn FFmpeg process")?;
    
    // Read PCM data from FFmpeg stdout
    let mut pcm_data = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_end(&mut pcm_data)
            .with_context(|| "Failed to read audio data from FFmpeg")?;
    }
    
    // Wait for FFmpeg to finish
    let status = child.wait()
        .with_context(|| "Failed to wait for FFmpeg process")?;
    
    if !status.success() {
        anyhow::bail!("FFmpeg failed to extract audio from TS file");
    }
    
    // Parse raw PCM data
    parse_pcm_from_memory(&pcm_data)
}

/// Parse raw PCM data from memory buffer
fn parse_pcm_from_memory(data: &[u8]) -> Result<AudioData> {
    // Convert raw PCM bytes to i16 samples
    let sample_count = data.len() / 2; // 2 bytes per i16 sample
    let mut samples = Vec::with_capacity(sample_count);
    
    for chunk in data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(sample as f32 / i16::MAX as f32);
    }
    
    // FFmpeg was configured for 16kHz mono
    let sample_rate = 16000;
    let channels = 1;
    let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u32;
    
    Ok(AudioData {
        samples,
        sample_rate,
        channels,
        duration_ms,
    })
}
