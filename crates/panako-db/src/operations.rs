use anyhow::{Context, Result};
use deadpool_postgres::Pool;

use crate::models::*;

/// Insert new fingerprint metadata
pub async fn insert_metadata(
    pool: &Pool,
    metadata: &NewFingerprintMetadata,
) -> Result<i32> {
    let client = pool.get().await?;
    
    let row = client
        .query_one(
            "INSERT INTO fingerprint_metadata 
             (original_path, filename, sample_rate, duration_ms, channels) 
             VALUES ($1, $2, $3, $4, $5) 
             RETURNING id",
            &[
                &metadata.original_path,
                &metadata.filename,
                &metadata.sample_rate,
                &metadata.duration_ms,
                &metadata.channels,
            ],
        )
        .await
        .context("Failed to insert fingerprint metadata")?;
    
    Ok(row.get(0))
}

/// Insert segmentation configuration
pub async fn insert_segmentation_config(
    pool: &Pool,
    config: &NewSegmentationConfig,
) -> Result<i32> {
    let client = pool.get().await?;
    
    let row = client
        .query_one(
            "INSERT INTO segmentation_config 
             (metadata_id, enabled, segment_duration_ms, overlap_ms) 
             VALUES ($1, $2, $3, $4) 
             RETURNING id",
            &[
                &config.metadata_id,
                &config.enabled,
                &config.segment_duration_ms,
                &config.overlap_ms,
            ],
        )
        .await
        .context("Failed to insert segmentation config")?;
    
    Ok(row.get(0))
}

/// Insert a segment
pub async fn insert_segment(pool: &Pool, segment: &NewSegment) -> Result<i32> {
    let client = pool.get().await?;
    
    let row = client
        .query_one(
            "INSERT INTO segments 
             (metadata_id, segment_index, start_ms, end_ms) 
             VALUES ($1, $2, $3, $4) 
             RETURNING id",
            &[
                &segment.metadata_id,
                &segment.segment_index,
                &segment.start_ms,
                &segment.end_ms,
            ],
        )
        .await
        .context("Failed to insert segment")?;
    
    Ok(row.get(0))
}

/// Batch insert fingerprints using JSONB
pub async fn insert_fingerprints_batch(
    pool: &Pool,
    fingerprints: &[NewFingerprint],
) -> Result<()> {
    if fingerprints.is_empty() {
        return Ok(());
    }
    
    let client = pool.get().await?;
    
    // Build the JSONB array
    let json_array = serde_json::to_value(fingerprints)
        .context("Failed to serialize fingerprints")?;
    
    client
        .execute(
            "INSERT INTO fingerprints (metadata_id, segment_id, hash, t1, f1, m1)
             SELECT 
                 (fp->>'metadata_id')::INTEGER,
                 (fp->>'segment_id')::INTEGER,
                 (fp->>'hash')::BIGINT,
                 (fp->>'t1')::INTEGER,
                 (fp->>'f1')::SMALLINT,
                 (fp->>'m1')::REAL
             FROM jsonb_array_elements($1::jsonb) AS fp",
            &[&json_array],
        )
        .await
        .context("Failed to batch insert fingerprints")?;
    
    Ok(())
}

/// Get metadata by ID
pub async fn get_metadata_by_id(pool: &Pool, id: i32) -> Result<Option<FingerprintMetadata>> {
    let client = pool.get().await?;
    
    let row = client
        .query_opt(
            "SELECT id, original_path, filename, sample_rate, duration_ms, channels, created_at 
             FROM fingerprint_metadata 
             WHERE id = $1",
            &[&id],
        )
        .await
        .context("Failed to get metadata")?;
    
    Ok(row.map(|r| FingerprintMetadata {
        id: r.get(0),
        original_path: r.get(1),
        filename: r.get(2),
        sample_rate: r.get(3),
        duration_ms: r.get(4),
        channels: r.get(5),
        created_at: r.get(6),
    }))
}

/// Get metadata by filename
pub async fn get_metadata_by_filename(
    pool: &Pool,
    filename: &str,
) -> Result<Option<FingerprintMetadata>> {
    let client = pool.get().await?;
    
    let row = client
        .query_opt(
            "SELECT id, original_path, filename, sample_rate, duration_ms, channels, created_at 
             FROM fingerprint_metadata 
             WHERE filename = $1",
            &[&filename],
        )
        .await
        .context("Failed to get metadata by filename")?;
    
    Ok(row.map(|r| FingerprintMetadata {
        id: r.get(0),
        original_path: r.get(1),
        filename: r.get(2),
        sample_rate: r.get(3),
        duration_ms: r.get(4),
        channels: r.get(5),
        created_at: r.get(6),
    }))
}

/// Get all metadata
pub async fn get_all_metadata(pool: &Pool) -> Result<Vec<FingerprintMetadata>> {
    let client = pool.get().await?;
    
    let rows = client
        .query(
            "SELECT id, original_path, filename, sample_rate, duration_ms, channels, created_at 
             FROM fingerprint_metadata 
             ORDER BY created_at DESC",
            &[],
        )
        .await
        .context("Failed to get all metadata")?;
    
    Ok(rows
        .iter()
        .map(|r| FingerprintMetadata {
            id: r.get(0),
            original_path: r.get(1),
            filename: r.get(2),
            sample_rate: r.get(3),
            duration_ms: r.get(4),
            channels: r.get(5),
            created_at: r.get(6),
        })
        .collect())
}

/// Get segments for a metadata ID
pub async fn get_segments_by_metadata(pool: &Pool, metadata_id: i32) -> Result<Vec<Segment>> {
    let client = pool.get().await?;
    
    let rows = client
        .query(
            "SELECT id, metadata_id, segment_index, start_ms, end_ms 
             FROM segments 
             WHERE metadata_id = $1 
             ORDER BY segment_index",
            &[&metadata_id],
        )
        .await
        .context("Failed to get segments")?;
    
    Ok(rows
        .iter()
        .map(|r| Segment {
            id: r.get(0),
            metadata_id: r.get(1),
            segment_index: r.get(2),
            start_ms: r.get(3),
            end_ms: r.get(4),
        })
        .collect())
}

/// Query fingerprints with criteria
pub async fn query_fingerprints(
    pool: &Pool,
    query: &FingerprintQuery,
) -> Result<Vec<Fingerprint>> {
    let client = pool.get().await?;
    
    let mut sql = String::from(
        "SELECT id, metadata_id, segment_id, hash, t1, f1, m1 
         FROM fingerprints 
         WHERE 1=1",
    );
    
    let mut param_count = 0;
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
    
    if let Some(ref metadata_id) = query.metadata_id {
        param_count += 1;
        sql.push_str(&format!(" AND metadata_id = ${}", param_count));
        params.push(metadata_id);
    }
    
    if let Some(ref segment_id) = query.segment_id {
        param_count += 1;
        sql.push_str(&format!(" AND segment_id = ${}", param_count));
        params.push(segment_id);
    }
    
    if let Some(ref hash) = query.hash {
        param_count += 1;
        sql.push_str(&format!(" AND hash = ${}", param_count));
        params.push(hash);
    }
    
    if let Some(ref limit) = query.limit {
        param_count += 1;
        sql.push_str(&format!(" LIMIT ${}", param_count));
        params.push(limit);
    }
    
    let rows = client
        .query(&sql, &params[..])
        .await
        .context("Failed to query fingerprints")?;
    
    Ok(rows
        .iter()
        .map(|r| Fingerprint {
            id: r.get(0),
            metadata_id: r.get(1),
            segment_id: r.get(2),
            hash: r.get(3),
            t1: r.get(4),
            f1: r.get(5),
            m1: r.get(6),
        })
        .collect())
}

/// Get fingerprints by hash (optimized query using the index)
pub async fn get_fingerprints_by_hash(pool: &Pool, hash: i64) -> Result<Vec<Fingerprint>> {
    let client = pool.get().await?;
    
    let rows = client
        .query(
            "SELECT id, metadata_id, segment_id, hash, t1, f1, m1 
             FROM fingerprints 
             WHERE hash = $1",
            &[&hash],
        )
        .await
        .context("Failed to get fingerprints by hash")?;
    
    Ok(rows
        .iter()
        .map(|r| Fingerprint {
            id: r.get(0),
            metadata_id: r.get(1),
            segment_id: r.get(2),
            hash: r.get(3),
            t1: r.get(4),
            f1: r.get(5),
            m1: r.get(6),
        })
        .collect())
}

/// Get all fingerprints for a metadata ID
pub async fn get_fingerprints_by_metadata(
    pool: &Pool,
    metadata_id: i32,
) -> Result<Vec<Fingerprint>> {
    let query = FingerprintQuery {
        metadata_id: Some(metadata_id),
        ..Default::default()
    };
    query_fingerprints(pool, &query).await
}

/// Delete metadata and all associated data (cascades)
pub async fn delete_metadata(pool: &Pool, metadata_id: i32) -> Result<()> {
    let client = pool.get().await?;
    
    client
        .execute(
            "DELETE FROM fingerprint_metadata WHERE id = $1",
            &[&metadata_id],
        )
        .await
        .context("Failed to delete metadata")?;
    
    Ok(())
}

/// Get summary information for all fingerprints
pub async fn get_fingerprint_summaries(pool: &Pool) -> Result<Vec<FingerprintSummary>> {
    let client = pool.get().await?;
    
    let rows = client
        .query(
            "SELECT * FROM fingerprint_summary ORDER BY filename",
            &[],
        )
        .await
        .context("Failed to get fingerprint summaries")?;
    
    Ok(rows
        .iter()
        .map(|r| FingerprintSummary {
            metadata_id: r.get(0),
            filename: r.get(1),
            duration_ms: r.get(2),
            total_segments: r.get(3),
            total_fingerprints: r.get(4),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    // Note: These tests require a running PostgreSQL instance
    // They are integration tests and should be run with:
    // cargo test --package panako-db -- --ignored
    
    #[tokio::test]
    #[ignore]
    async fn test_insert_and_retrieve_metadata() {
        // This would require a test database setup
        // Left as a placeholder for future integration tests
    }
}
