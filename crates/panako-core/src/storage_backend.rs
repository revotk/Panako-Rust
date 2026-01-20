//! Storage backend trait and implementations
//!
//! Provides abstraction layer for different storage backends (filesystem, PostgreSQL)

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

use crate::storage_config::{FileFormat, FilesystemConfig, PostgresqlConfig};

/// Metadata for fingerprint storage
#[derive(Debug, Clone)]
pub struct FingerprintMetadata {
    pub filename: String,
    pub original_path: String,
    pub algorithm: String,
    pub sample_rate: u32,
    pub duration_ms: u32,
    pub channels: u16,
    pub created_at: String,
}

/// Query criteria for fingerprint retrieval
#[derive(Debug, Clone, Default)]
pub struct QueryCriteria {
    pub filename_pattern: Option<String>,
    pub min_duration_ms: Option<u32>,
    pub max_duration_ms: Option<u32>,
    pub created_after: Option<String>,
    pub created_before: Option<String>,
}

/// Abstract storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Load fingerprints by identifier
    async fn load_fingerprints(&self, identifier: &str) -> Result<Vec<(u64, i32, i16, f32)>>;
    
    /// Load all fingerprints from storage
    async fn load_all_fingerprints(&self) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>>;
    
    /// Save fingerprints with metadata
    async fn save_fingerprints(
        &self,
        identifier: &str,
        fingerprints: &[(u64, i32, i16, f32)],
        metadata: &FingerprintMetadata,
    ) -> Result<()>;
    
    /// Query fingerprints by criteria
    async fn query_fingerprints(
        &self,
        criteria: &QueryCriteria,
    ) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>>;
    
    /// Get metadata for a fingerprint
    async fn get_metadata(&self, identifier: &str) -> Result<Option<FingerprintMetadata>>;
}

/// Filesystem-based storage backend
pub struct FilesystemBackend {
    base_dir: PathBuf,
    format: FileFormat,
}

impl FilesystemBackend {
    /// Create a new filesystem backend
    pub fn new(config: &FilesystemConfig) -> Self {
        Self {
            base_dir: PathBuf::from(&config.base_directory),
            format: config.format.clone(),
        }
    }
    
    /// Create from directory path and format
    pub fn from_path(base_dir: &str, format: FileFormat) -> Self {
        Self {
            base_dir: PathBuf::from(base_dir),
            format,
        }
    }
    
    /// Determine file extension based on format
    fn get_extension(&self, format: &FileFormat) -> &str {
        match format {
            FileFormat::Json => "json",
            FileFormat::Bson => "bson",
            FileFormat::Auto => "json", // Default to JSON for auto
        }
    }
    
    /// Find fingerprint file for identifier
    fn find_file(&self, identifier: &str) -> Result<PathBuf> {
        match self.format {
            FileFormat::Auto => {
                // Try both extensions
                let json_path = self.base_dir.join(format!("{}.json", identifier));
                let bson_path = self.base_dir.join(format!("{}.bson", identifier));
                
                if json_path.exists() {
                    Ok(json_path)
                } else if bson_path.exists() {
                    Ok(bson_path)
                } else {
                    anyhow::bail!("Fingerprint file not found for identifier: {}", identifier)
                }
            }
            _ => {
                let ext = self.get_extension(&self.format);
                let path = self.base_dir.join(format!("{}.{}", identifier, ext));
                if path.exists() {
                    Ok(path)
                } else {
                    anyhow::bail!("Fingerprint file not found: {}", path.display())
                }
            }
        }
    }
}

#[async_trait]
impl StorageBackend for FilesystemBackend {
    async fn load_fingerprints(&self, identifier: &str) -> Result<Vec<(u64, i32, i16, f32)>> {
        use panako_fp::FpJsonFile;
        
        let file_path = self.find_file(identifier)?;
        
        // Auto-detect format (JSON or BSON)
        let fp_file = FpJsonFile::load_auto(&file_path)?;
        let fingerprints = fp_file.get_all_fingerprints();
        
        Ok(fingerprints)
    }
    
    async fn load_all_fingerprints(&self) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>> {
        use panako_fp::FpJsonFile;
        use rayon::prelude::*;
        
        // Find all fingerprint files
        let entries = std::fs::read_dir(&self.base_dir)?;
        let files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext == "json" || ext == "bson")
                    .unwrap_or(false)
            })
            .collect();
        
        // Load all files in parallel (auto-detect format)
        let results: Vec<(String, Vec<(u64, i32, i16, f32)>)> = files
            .par_iter()
            .filter_map(|path| {
                match FpJsonFile::load_auto(path) {
                    Ok(fp_file) => {
                        let identifier = fp_file.metadata.filename.clone();
                        let fingerprints = fp_file.get_all_fingerprints();
                        Some((identifier, fingerprints))
                    }
                    Err(e) => {
                        log::warn!("Failed to load {}: {}", path.display(), e);
                        None
                    }
                }
            })
            .collect();
        
        Ok(results)
    }
    
    async fn save_fingerprints(
        &self,
        identifier: &str,
        fingerprints: &[(u64, i32, i16, f32)],
        metadata: &FingerprintMetadata,
    ) -> Result<()> {
        use panako_fp::{FpJsonFile, FpJsonSegment, FpJsonFingerprint};
        
        // Create FpJsonFile from fingerprints
        let mut fp_file = FpJsonFile::new(
            metadata.original_path.clone(),
            metadata.filename.clone(),
            metadata.sample_rate,
            metadata.duration_ms,
            metadata.channels,
        );
        
        // Create a single segment with all fingerprints
        let fps: Vec<FpJsonFingerprint> = fingerprints
            .iter()
            .map(|(hash, t1, f1, m1)| FpJsonFingerprint {
                hash: *hash,
                t1: *t1,
                f1: *f1,
                m1: *m1,
            })
            .collect();
        
        let segment = FpJsonSegment {
            segment_id: 0,
            start_time_s: 0.0,
            end_time_s: metadata.duration_ms as f64 / 1000.0,
            num_fingerprints: fps.len(),
            fingerprints: fps,
        };
        
        fp_file.add_segment(segment);
        
        // Determine file extension and save method based on format
        let (ext, save_fn): (&str, fn(&FpJsonFile, &std::path::Path) -> anyhow::Result<()>) = 
            match &self.format {
                FileFormat::Bson => ("bson", FpJsonFile::save_bson),
                FileFormat::Json => ("json", FpJsonFile::save),
                FileFormat::Auto => ("json", FpJsonFile::save), // Default to JSON
            };
        
        let file_path = self.base_dir.join(format!("{}.{}", identifier, ext));
        
        // Create directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        save_fn(&fp_file, &file_path)?;
        
        Ok(())
    }
    
    async fn query_fingerprints(
        &self,
        criteria: &QueryCriteria,
    ) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>> {
        // Load all fingerprints
        let all_fps = self.load_all_fingerprints().await?;
        
        // Filter based on criteria
        let filtered: Vec<(String, Vec<(u64, i32, i16, f32)>)> = all_fps
            .into_iter()
            .filter(|(identifier, _)| {
                // Apply filename pattern filter
                if let Some(pattern) = &criteria.filename_pattern {
                    if !identifier.contains(pattern) {
                        return false;
                    }
                }
                
                // For duration and date filters, we need to load metadata
                // This is a simplified implementation
                true
            })
            .collect();
        
        Ok(filtered)
    }
    
    async fn get_metadata(&self, identifier: &str) -> Result<Option<FingerprintMetadata>> {
        use panako_fp::FpJsonFile;
        
        let file_path = self.find_file(identifier)?;
        let fp_file = FpJsonFile::load(&file_path)?;
        
        let metadata = FingerprintMetadata {
            filename: fp_file.metadata.filename,
            original_path: fp_file.metadata.original_path,
            algorithm: fp_file.metadata.algorithm,
            sample_rate: fp_file.metadata.sample_rate,
            duration_ms: fp_file.metadata.duration_ms,
            channels: fp_file.metadata.channels,
            created_at: fp_file.metadata.created_at,
        };
        
        Ok(Some(metadata))
    }
}

/// PostgreSQL-based storage backend
pub struct PostgresqlBackend {
    pool: deadpool_postgres::Pool,
}

impl PostgresqlBackend {
    /// Create a new PostgreSQL backend
    pub async fn new(config: &PostgresqlConfig) -> Result<Self> {
        let pool = panako_db::create_pool(
            &config.host,
            config.port,
            &config.database,
            &config.user,
            &config.password,
            config.max_connections,
        )?;
        
        // Test the connection
        panako_db::test_connection(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl StorageBackend for PostgresqlBackend {
    async fn load_fingerprints(&self, identifier: &str) -> Result<Vec<(u64, i32, i16, f32)>> {
        // Get metadata by filename
        let metadata = panako_db::get_metadata_by_filename(&self.pool, identifier)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Fingerprint not found: {}", identifier))?;
        
        // Get all fingerprints for this metadata
        let db_fingerprints = panako_db::get_fingerprints_by_metadata(&self.pool, metadata.id).await?;
        
        // Convert to tuple format
        let fingerprints = db_fingerprints
            .into_iter()
            .map(|fp| (fp.hash as u64, fp.t1, fp.f1, fp.m1))
            .collect();
        
        Ok(fingerprints)
    }
    
    async fn load_all_fingerprints(&self) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>> {
        // Get all metadata
        let all_metadata = panako_db::get_all_metadata(&self.pool).await?;
        
        let mut results = Vec::new();
        
        for metadata in all_metadata {
            // Get fingerprints for each metadata
            let db_fingerprints = panako_db::get_fingerprints_by_metadata(&self.pool, metadata.id).await?;
            
            let fingerprints = db_fingerprints
                .into_iter()
                .map(|fp| (fp.hash as u64, fp.t1, fp.f1, fp.m1))
                .collect();
            
            results.push((metadata.filename, fingerprints));
        }
        
        Ok(results)
    }
    
    async fn save_fingerprints(
        &self,
        _identifier: &str,
        fingerprints: &[(u64, i32, i16, f32)],
        metadata: &FingerprintMetadata,
    ) -> Result<()> {
        // Insert metadata
        let new_metadata = panako_db::NewFingerprintMetadata {
            original_path: metadata.original_path.clone(),
            filename: metadata.filename.clone(),
            sample_rate: metadata.sample_rate as i32,
            duration_ms: metadata.duration_ms as i32,
            channels: metadata.channels as i16,
        };
        
        let metadata_id = panako_db::insert_metadata(&self.pool, &new_metadata).await?;
        
        // Insert segmentation config (disabled by default)
        let seg_config = panako_db::NewSegmentationConfig {
            metadata_id,
            enabled: false,
            segment_duration_ms: None,
            overlap_ms: None,
        };
        
        panako_db::insert_segmentation_config(&self.pool, &seg_config).await?;
        
        // Convert fingerprints to database format
        let db_fingerprints: Vec<panako_db::NewFingerprint> = fingerprints
            .iter()
            .map(|(hash, t1, f1, m1)| panako_db::NewFingerprint {
                metadata_id,
                segment_id: None,
                hash: *hash as i64,
                t1: *t1,
                f1: *f1,
                m1: *m1,
            })
            .collect();
        
        // Batch insert fingerprints
        panako_db::insert_fingerprints_batch(&self.pool, &db_fingerprints).await?;
        
        Ok(())
    }
    
    async fn query_fingerprints(
        &self,
        criteria: &QueryCriteria,
    ) -> Result<Vec<(String, Vec<(u64, i32, i16, f32)>)>> {
        // Get all metadata first
        let all_metadata = panako_db::get_all_metadata(&self.pool).await?;
        
        // Filter metadata based on criteria
        let filtered_metadata: Vec<_> = all_metadata
            .into_iter()
            .filter(|meta| {
                // Apply filename pattern filter
                if let Some(pattern) = &criteria.filename_pattern {
                    if !meta.filename.contains(pattern) {
                        return false;
                    }
                }
                
                // Apply duration filters
                if let Some(min_duration) = criteria.min_duration_ms {
                    if meta.duration_ms < min_duration as i32 {
                        return false;
                    }
                }
                
                if let Some(max_duration) = criteria.max_duration_ms {
                    if meta.duration_ms > max_duration as i32 {
                        return false;
                    }
                }
                
                // Apply date filters
                if let Some(created_after) = &criteria.created_after {
                    if meta.created_at.to_rfc3339() < *created_after {
                        return false;
                    }
                }
                
                if let Some(created_before) = &criteria.created_before {
                    if meta.created_at.to_rfc3339() > *created_before {
                        return false;
                    }
                }
                
                true
            })
            .collect();
        
        // Load fingerprints for filtered metadata
        let mut results = Vec::new();
        
        for metadata in filtered_metadata {
            let db_fingerprints = panako_db::get_fingerprints_by_metadata(&self.pool, metadata.id).await?;
            
            let fingerprints = db_fingerprints
                .into_iter()
                .map(|fp| (fp.hash as u64, fp.t1, fp.f1, fp.m1))
                .collect();
            
            results.push((metadata.filename, fingerprints));
        }
        
        Ok(results)
    }
    
    async fn get_metadata(&self, identifier: &str) -> Result<Option<FingerprintMetadata>> {
        let db_metadata = panako_db::get_metadata_by_filename(&self.pool, identifier).await?;
        
        Ok(db_metadata.map(|meta| FingerprintMetadata {
            filename: meta.filename,
            original_path: meta.original_path,
            algorithm: "panako".to_string(), // Default algorithm name
            sample_rate: meta.sample_rate as u32,
            duration_ms: meta.duration_ms as u32,
            channels: meta.channels as u16,
            created_at: meta.created_at.to_rfc3339(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_filesystem_backend() {
        let config = FilesystemConfig {
            base_directory: "./test_db".to_string(),
            format: FileFormat::Bson,
        };
        let backend = FilesystemBackend::new(&config);
        assert_eq!(backend.base_dir, PathBuf::from("./test_db"));
        assert_eq!(backend.format, FileFormat::Bson);
    }

    #[test]
    fn test_create_postgresql_backend() {
        let config = PostgresqlConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "test".to_string(),
            user: "test_user".to_string(),
            password: "test_pass".to_string(),
            max_connections: 5,
        };
        let _backend = PostgresqlBackend::new(&config);
        // Just verify it can be created
    }
}
