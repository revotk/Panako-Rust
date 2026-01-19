//! .fp file writer

use crate::format::{FpFile, FpHeader, FpMetadata};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct FpWriter {}

impl FpWriter {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Write .fp file
    pub fn write(&self, path: &Path, fp_file: &FpFile) -> Result<()> {
        let file = File::create(path)
            .with_context(|| format!("Failed to create .fp file: {}", path.display()))?;
        
        let mut writer = BufWriter::new(file);
        
        // Write header (will update later with checksum)
        self.write_header(&mut writer, &fp_file.header)?;
        
        // Write metadata
        self.write_metadata(&mut writer, &fp_file.metadata)?;
        
        // Write fingerprints
        self.write_fingerprints(&mut writer, &fp_file.fingerprints)?;
        
        writer.flush()?;
        
        Ok(())
    }
    
    fn write_header(&self, writer: &mut BufWriter<File>, header: &FpHeader) -> Result<()> {
        // Write as little-endian binary
        writer.write_all(&header.magic)?;
        writer.write_all(&header.version.to_le_bytes())?;
        writer.write_all(&header.flags.to_le_bytes())?;
        writer.write_all(&header.metadata_size.to_le_bytes())?;
        writer.write_all(&header.payload_size.to_le_bytes())?;
        writer.write_all(&header.payload_size_compressed.to_le_bytes())?;
        writer.write_all(&header.num_fingerprints.to_le_bytes())?;
        writer.write_all(&header.sample_rate.to_le_bytes())?;
        writer.write_all(&header.duration_ms.to_le_bytes())?;
        writer.write_all(&header.channels.to_le_bytes())?;
        writer.write_all(&header.reserved1.to_le_bytes())?;
        writer.write_all(&header.checksum.to_le_bytes())?;
        writer.write_all(&header.reserved2.to_le_bytes())?;
        
        Ok(())
    }
    
    fn write_metadata(&self, writer: &mut BufWriter<File>, metadata: &FpMetadata) -> Result<()> {
        // Write algorithm ID (8 bytes, null-padded)
        let mut algo_id = [0u8; 8];
        let bytes = metadata.algorithm_id.as_bytes();
        let len = bytes.len().min(8);
        algo_id[..len].copy_from_slice(&bytes[..len]);
        writer.write_all(&algo_id)?;
        
        // Write algorithm params as JSON (length-prefixed)
        let params_bytes = metadata.algorithm_params.as_bytes();
        writer.write_all(&(params_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(params_bytes)?;
        
        // Write original filename (null-terminated)
        writer.write_all(metadata.original_filename.as_bytes())?;
        writer.write_all(&[0])?;
        
        Ok(())
    }
    
    fn write_fingerprints(
        &self,
        writer: &mut BufWriter<File>,
        fingerprints: &[(u64, i32, i16, f32)],
    ) -> Result<()> {
        // Write each fingerprint: 20 bytes
        for (hash, t1, f1, m1) in fingerprints {
            writer.write_all(&hash.to_le_bytes())?;
            writer.write_all(&t1.to_le_bytes())?;
            writer.write_all(&f1.to_le_bytes())?;
            writer.write_all(&0u16.to_le_bytes())?; // padding
            writer.write_all(&m1.to_le_bytes())?;
        }
        
        Ok(())
    }
}

impl Default for FpWriter {
    fn default() -> Self {
        Self::new()
    }
}
