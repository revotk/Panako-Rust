//! .fp file reader

use crate::format::{FpFile, FpHeader, FpMetadata, MAGIC};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct FpReader;

impl FpReader {
    /// Read .fp file
    pub fn read(path: &Path) -> Result<FpFile> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open .fp file: {}", path.display()))?;
        
        let mut reader = BufReader::new(file);
        
        // Read header
        let header = Self::read_header(&mut reader)?;
        
        // Validate magic
        if header.magic != MAGIC {
            anyhow::bail!("Invalid .fp file: magic bytes mismatch");
        }
        
        // Read metadata
        let metadata = Self::read_metadata(&mut reader, header.metadata_size as usize)?;
        
        // Read fingerprints
        let fingerprints = Self::read_fingerprints(&mut reader, header.num_fingerprints as usize)?;
        
        Ok(FpFile {
            header,
            metadata,
            fingerprints,
        })
    }
    
    fn read_header(reader: &mut BufReader<File>) -> Result<FpHeader> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        let version = Self::read_u16(reader)?;
        let flags = Self::read_u16(reader)?;
        let metadata_size = Self::read_u64(reader)?;
        let payload_size = Self::read_u64(reader)?;
        let payload_size_compressed = Self::read_u64(reader)?;
        let num_fingerprints = Self::read_u32(reader)?;
        let sample_rate = Self::read_u32(reader)?;
        let duration_ms = Self::read_u32(reader)?;
        let channels = Self::read_u16(reader)?;
        let reserved1 = Self::read_u16(reader)?;
        let checksum = Self::read_u64(reader)?;
        let reserved2 = Self::read_u64(reader)?;
        
        Ok(FpHeader {
            magic,
            version,
            flags,
            metadata_size,
            payload_size,
            payload_size_compressed,
            num_fingerprints,
            sample_rate,
            duration_ms,
            channels,
            reserved1,
            checksum,
            reserved2,
        })
    }
    
    fn read_metadata(reader: &mut BufReader<File>, _size: usize) -> Result<FpMetadata> {
        // Read algorithm ID (8 bytes)
        let mut algo_id = [0u8; 8];
        reader.read_exact(&mut algo_id)?;
        let algorithm_id = String::from_utf8_lossy(&algo_id)
            .trim_end_matches('\0')
            .to_string();
        
        // Read algorithm params (length-prefixed)
        let params_len = Self::read_u32(reader)? as usize;
        let mut params_bytes = vec![0u8; params_len];
        reader.read_exact(&mut params_bytes)?;
        let algorithm_params = String::from_utf8(params_bytes)?;
        
        // Read original filename (null-terminated)
        let mut filename_bytes = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            filename_bytes.push(byte[0]);
        }
        let original_filename = String::from_utf8(filename_bytes)?;
        
        Ok(FpMetadata {
            algorithm_id,
            algorithm_params,
            original_filename,
            segmentation: None,  // Will be populated from JSON if present
        })
    }
    
    fn read_fingerprints(
        reader: &mut BufReader<File>,
        count: usize,
    ) -> Result<Vec<(u64, i32, i16, f32)>> {
        let mut fingerprints = Vec::with_capacity(count);
        
        for _ in 0..count {
            let hash = Self::read_u64(reader)?;
            let t1 = Self::read_i32(reader)?;
            let f1 = Self::read_i16(reader)?;
            let _padding = Self::read_u16(reader)?;
            let m1 = Self::read_f32(reader)?;
            
            fingerprints.push((hash, t1, f1, m1));
        }
        
        Ok(fingerprints)
    }
    
    fn read_u16(reader: &mut BufReader<File>) -> Result<u16> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    
    fn read_u32(reader: &mut BufReader<File>) -> Result<u32> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    
    fn read_u64(reader: &mut BufReader<File>) -> Result<u64> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
    
    fn read_i16(reader: &mut BufReader<File>) -> Result<i16> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
    
    fn read_i32(reader: &mut BufReader<File>) -> Result<i32> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
    
    fn read_f32(reader: &mut BufReader<File>) -> Result<f32> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }
}
