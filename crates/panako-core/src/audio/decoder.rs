//! Audio decoding for multiple formats

use super::{resample_to_target, AudioFormat};
use anyhow::{Context, Result};
use std::path::Path;

/// Decoded audio data
#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u32,
}

impl AudioData {
    /// Convert to mono by averaging channels
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }
        
        let mut mono = Vec::with_capacity(self.samples.len() / self.channels as usize);
        for chunk in self.samples.chunks(self.channels as usize) {
            let avg: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
            mono.push(avg);
        }
        mono
    }
}

/// Decode audio file to target sample rate
pub fn decode_audio(path: &str, target_sample_rate: u32) -> Result<AudioData> {
    let path = Path::new(path);
    
    if !path.exists() {
        anyhow::bail!("Audio file not found: {}", path.display());
    }
    
    let format = AudioFormat::from_path(path);
    
    // Handle MPEG-TS separately (Symphonia doesn't support TS)
    if format == AudioFormat::MpegTs {
        let mut audio_data = super::extract_audio_from_ts(path)?;
        
        // Resample if needed
        if audio_data.sample_rate != target_sample_rate {
            let mono = audio_data.to_mono();
            let resampled = resample_to_target(&mono, audio_data.sample_rate, target_sample_rate)?;
            audio_data.samples = resampled;
            audio_data.sample_rate = target_sample_rate;
            audio_data.channels = 1;
        } else if audio_data.channels > 1 {
            audio_data.samples = audio_data.to_mono();
            audio_data.channels = 1;
        }
        
        return Ok(audio_data);
    }
    
    // Handle video formats with Symphonia
    let mut audio_data = if format.is_video_container() {
        super::extract_audio_from_video(path)?
    } else {
        // Handle pure audio formats
        match format {
            AudioFormat::Wav => decode_wav(path)?,
            AudioFormat::Mp3 => decode_mp3(path)?,
            AudioFormat::Flac => decode_flac(path)?,
            AudioFormat::Ogg => decode_ogg(path)?,
            AudioFormat::Unknown => {
                anyhow::bail!("Unsupported audio format: {}", path.display());
            }
            _ => {
                // Fallback: try Symphonia for any other format
                super::extract_audio_from_video(path)?
            }
        }
    };
    
    // Resample if needed
    if audio_data.sample_rate != target_sample_rate {
        let mono = audio_data.to_mono();
        let resampled = resample_to_target(&mono, audio_data.sample_rate, target_sample_rate)?;
        audio_data.samples = resampled;
        audio_data.sample_rate = target_sample_rate;
        audio_data.channels = 1;
    } else if audio_data.channels > 1 {
        // Convert to mono even if sample rate matches
        audio_data.samples = audio_data.to_mono();
        audio_data.channels = 1;
    }
    
    Ok(audio_data)
}

/// Decode WAV file
fn decode_wav(path: &Path) -> Result<AudioData> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;
    
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    
    // Read samples and convert to f32
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?
        }
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    
    let duration_ms = (samples.len() as f64 / (sample_rate * channels as u32) as f64 * 1000.0) as u32;
    
    Ok(AudioData {
        samples,
        sample_rate,
        channels,
        duration_ms,
    })
}

/// Decode MP3 file
fn decode_mp3(path: &Path) -> Result<AudioData> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read MP3 file: {}", path.display()))?;
    
    let mut decoder = minimp3::Decoder::new(&data[..]);
    let mut samples = Vec::new();
    let mut sample_rate = 0;
    let mut channels = 0;
    
    loop {
        match decoder.next_frame() {
            Ok(frame) => {
                if sample_rate == 0 {
                    sample_rate = frame.sample_rate as u32;
                    channels = frame.channels as u16;
                }
                // Convert i16 to f32
                for &sample in &frame.data {
                    samples.push(sample as f32 / 32768.0);
                }
            }
            Err(minimp3::Error::Eof) => break,
            Err(e) => anyhow::bail!("MP3 decode error: {}", e),
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

/// Decode FLAC file
fn decode_flac(path: &Path) -> Result<AudioData> {
    let mut reader = claxon::FlacReader::open(path)
        .with_context(|| format!("Failed to open FLAC file: {}", path.display()))?;
    
    let info = reader.streaminfo();
    let sample_rate = info.sample_rate;
    let channels = info.channels as u16;
    let bits_per_sample = info.bits_per_sample;
    
    let max_val = (1i64 << (bits_per_sample - 1)) as f32;
    let samples: Vec<f32> = reader
        .samples()
        .map(|s| s.map(|v| v as f32 / max_val))
        .collect::<Result<Vec<_>, _>>()?;
    
    let duration_ms = (samples.len() as f64 / (sample_rate * channels as u32) as f64 * 1000.0) as u32;
    
    Ok(AudioData {
        samples,
        sample_rate,
        channels,
        duration_ms,
    })
}

/// Decode OGG Vorbis file
fn decode_ogg(path: &Path) -> Result<AudioData> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open OGG file: {}", path.display()))?;
    
    let mut reader = lewton::inside_ogg::OggStreamReader::new(file)?;
    
    let sample_rate = reader.ident_hdr.audio_sample_rate;
    let channels = reader.ident_hdr.audio_channels as u16;
    
    let mut samples = Vec::new();
    
    while let Some(packet) = reader.read_dec_packet_itl()? {
        // Convert i16 to f32
        for &sample in &packet {
            samples.push(sample as f32 / 32768.0);
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


