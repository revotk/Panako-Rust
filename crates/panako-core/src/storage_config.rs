//! Storage configuration for Panako
//!
//! Provides TOML-based configuration for selecting storage backend
//! (filesystem vs PostgreSQL) and related parameters.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanakoStorageConfig {
    pub storage: StorageConfig,
    #[serde(default)]
    pub matching: MatchingConfig,
    #[serde(default)]
    pub segmentation: SegmentationConfig,
}

/// Storage backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    #[serde(default)]
    pub filesystem: FilesystemConfig,
    #[serde(default)]
    pub postgresql: PostgresqlConfig,
}

/// Storage backend type
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    Filesystem,
    Postgresql,
}

/// Filesystem backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FilesystemConfig {
    #[serde(default = "default_base_directory")]
    pub base_directory: String,
    #[serde(default)]
    pub format: FileFormat,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            base_directory: default_base_directory(),
            format: FileFormat::default(),
        }
    }
}

fn default_base_directory() -> String {
    "./fingerprints".to_string()
}

/// File format for filesystem storage
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Json,
    Bson,
    Auto, // Auto-detect based on file extension
}

impl Default for FileFormat {
    fn default() -> Self {
        FileFormat::Auto
    }
}

/// PostgreSQL backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresqlConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_database")]
    pub database: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default = "default_password")]
    pub password: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

impl Default for PostgresqlConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            database: default_database(),
            user: default_user(),
            password: default_password(),
            max_connections: default_max_connections(),
        }
    }
}

fn default_host() -> String {
    "localhost".to_string()
}
fn default_port() -> u16 {
    5432
}
fn default_database() -> String {
    "panako".to_string()
}
fn default_user() -> String {
    "panako_user".to_string()
}
fn default_password() -> String {
    "panako_pass".to_string()
}
fn default_max_connections() -> u32 {
    10
}

/// Matching configuration (extends existing PanakoConfig)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchingConfig {
    #[serde(default = "default_min_aligned_matches")]
    pub min_aligned_matches: usize,
    #[serde(default = "default_max_time_delta")]
    pub max_time_delta: i32,
    #[serde(default = "default_max_freq_delta")]
    pub max_freq_delta: i16,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            min_aligned_matches: default_min_aligned_matches(),
            max_time_delta: default_max_time_delta(),
            max_freq_delta: default_max_freq_delta(),
        }
    }
}

fn default_min_aligned_matches() -> usize {
    5
}
fn default_max_time_delta() -> i32 {
    3
}
fn default_max_freq_delta() -> i16 {
    128
}

/// Segmentation configuration (for -m flag)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SegmentationConfig {
    #[serde(default = "default_segment_duration")]
    pub segment_duration_s: f64,
    #[serde(default = "default_overlap_duration")]
    pub overlap_duration_s: f64,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            segment_duration_s: default_segment_duration(),
            overlap_duration_s: default_overlap_duration(),
        }
    }
}

fn default_segment_duration() -> f64 {
    25.0
}
fn default_overlap_duration() -> f64 {
    5.0
}

impl PanakoStorageConfig {
    /// Load configuration from TOML file
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e))?;
        let config: PanakoStorageConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;
        Ok(config)
    }

    /// Get PostgreSQL connection string
    pub fn connection_string(&self) -> Option<String> {
        match self.storage.backend {
            StorageBackend::Postgresql => {
                let pg = &self.storage.postgresql;
                Some(format!(
                    "postgresql://{}:{}@{}:{}/{}",
                    pg.user, pg.password, pg.host, pg.port, pg.database
                ))
            }
            _ => None,
        }
    }

    /// Create a default filesystem configuration
    pub fn default_filesystem() -> Self {
        Self {
            storage: StorageConfig {
                backend: StorageBackend::Filesystem,
                filesystem: FilesystemConfig::default(),
                postgresql: PostgresqlConfig::default(),
            },
            matching: MatchingConfig::default(),
            segmentation: SegmentationConfig::default(),
        }
    }

    /// Create a default PostgreSQL configuration
    pub fn default_postgresql() -> Self {
        Self {
            storage: StorageConfig {
                backend: StorageBackend::Postgresql,
                filesystem: FilesystemConfig::default(),
                postgresql: PostgresqlConfig::default(),
            },
            matching: MatchingConfig::default(),
            segmentation: SegmentationConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_filesystem_config() {
        let config = PanakoStorageConfig::default_filesystem();
        assert_eq!(config.storage.backend, StorageBackend::Filesystem);
        assert_eq!(config.storage.filesystem.base_directory, "./fingerprints");
    }

    #[test]
    fn test_default_postgresql_config() {
        let config = PanakoStorageConfig::default_postgresql();
        assert_eq!(config.storage.backend, StorageBackend::Postgresql);
        assert_eq!(config.storage.postgresql.host, "localhost");
        assert_eq!(config.storage.postgresql.port, 5432);
    }

    #[test]
    fn test_connection_string() {
        let config = PanakoStorageConfig::default_postgresql();
        let conn_str = config.connection_string().unwrap();
        assert!(conn_str.contains("postgresql://"));
        assert!(conn_str.contains("panako_user"));
        assert!(conn_str.contains("localhost:5432"));
    }

    #[test]
    fn test_parse_filesystem_toml() {
        let toml_str = r#"
            [storage]
            backend = "filesystem"
            
            [storage.filesystem]
            base_directory = "./test_db"
            format = "bson"
        "#;
        
        let config: PanakoStorageConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.storage.backend, StorageBackend::Filesystem);
        assert_eq!(config.storage.filesystem.base_directory, "./test_db");
        assert_eq!(config.storage.filesystem.format, FileFormat::Bson);
    }

    #[test]
    fn test_parse_postgresql_toml() {
        let toml_str = r#"
            [storage]
            backend = "postgresql"
            
            [storage.postgresql]
            host = "db.example.com"
            port = 5433
            database = "test_panako"
            user = "test_user"
            password = "test_pass"
            max_connections = 20
        "#;
        
        let config: PanakoStorageConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.storage.backend, StorageBackend::Postgresql);
        assert_eq!(config.storage.postgresql.host, "db.example.com");
        assert_eq!(config.storage.postgresql.port, 5433);
        assert_eq!(config.storage.postgresql.database, "test_panako");
    }
}
