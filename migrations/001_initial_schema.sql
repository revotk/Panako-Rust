-- Panako PostgreSQL Schema
-- Initial migration for fingerprint storage with JSONB

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Table: fingerprint_metadata
-- Stores metadata about audio files
CREATE TABLE IF NOT EXISTS fingerprint_metadata (
    id SERIAL PRIMARY KEY,
    filename VARCHAR(255) NOT NULL UNIQUE,
    original_path TEXT NOT NULL,
    algorithm VARCHAR(50) NOT NULL DEFAULT 'PANAKO',
    sample_rate INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    channels SMALLINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    format VARCHAR(20) NOT NULL DEFAULT 'json',
    file_size_bytes BIGINT,
    CONSTRAINT chk_sample_rate CHECK (sample_rate > 0),
    CONSTRAINT chk_duration CHECK (duration_ms > 0),
    CONSTRAINT chk_channels CHECK (channels > 0)
);

CREATE INDEX idx_filename ON fingerprint_metadata(filename);
CREATE INDEX idx_created_at ON fingerprint_metadata(created_at);
CREATE INDEX idx_duration ON fingerprint_metadata(duration_ms);

-- Table: segmentation_config
-- Stores segmentation configuration for each file
CREATE TABLE IF NOT EXISTS segmentation_config (
    metadata_id INTEGER PRIMARY KEY REFERENCES fingerprint_metadata(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    segment_duration_s DOUBLE PRECISION,
    overlap_duration_s DOUBLE PRECISION,
    num_segments INTEGER,
    CONSTRAINT chk_segment_duration CHECK (segment_duration_s IS NULL OR segment_duration_s > 0),
    CONSTRAINT chk_overlap CHECK (overlap_duration_s IS NULL OR overlap_duration_s >= 0)
);

-- Table: segments
-- Stores individual segments of audio files
CREATE TABLE IF NOT EXISTS segments (
    id SERIAL PRIMARY KEY,
    metadata_id INTEGER NOT NULL REFERENCES fingerprint_metadata(id) ON DELETE CASCADE,
    segment_id INTEGER NOT NULL,
    start_time_s DOUBLE PRECISION NOT NULL,
    end_time_s DOUBLE PRECISION NOT NULL,
    num_fingerprints INTEGER NOT NULL,
    CONSTRAINT chk_times CHECK (end_time_s > start_time_s),
    CONSTRAINT chk_num_fps CHECK (num_fingerprints >= 0),
    UNIQUE(metadata_id, segment_id)
);

CREATE INDEX idx_metadata_segment ON segments(metadata_id, segment_id);

-- Table: fingerprints
-- Stores fingerprints in JSONB format for flexibility
CREATE TABLE IF NOT EXISTS fingerprints (
    id SERIAL PRIMARY KEY,
    segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
    fingerprint_data JSONB NOT NULL,
    CONSTRAINT chk_fingerprint_data CHECK (jsonb_typeof(fingerprint_data) = 'array')
);

CREATE INDEX idx_segment ON fingerprints(segment_id);
CREATE INDEX idx_fingerprint_gin ON fingerprints USING GIN (fingerprint_data);

-- Table: fingerprint_index
-- Optimized index table for fast hash lookups
CREATE TABLE IF NOT EXISTS fingerprint_index (
    id SERIAL PRIMARY KEY,
    hash BIGINT NOT NULL,
    metadata_id INTEGER NOT NULL REFERENCES fingerprint_metadata(id) ON DELETE CASCADE,
    segment_id INTEGER NOT NULL,
    t1 INTEGER NOT NULL,
    f1 SMALLINT NOT NULL,
    m1 REAL NOT NULL
);

CREATE INDEX idx_hash ON fingerprint_index(hash);
CREATE INDEX idx_metadata_hash ON fingerprint_index(metadata_id, hash);
CREATE INDEX idx_hash_t1 ON fingerprint_index(hash, t1);

-- View: fingerprint_summary
-- Convenient view for querying fingerprint statistics
CREATE OR REPLACE VIEW fingerprint_summary AS
SELECT 
    m.id,
    m.filename,
    m.duration_ms,
    m.created_at,
    sc.enabled as segmentation_enabled,
    sc.num_segments,
    COUNT(DISTINCT s.id) as actual_segments,
    SUM(s.num_fingerprints) as total_fingerprints,
    COUNT(fi.id) as indexed_fingerprints
FROM fingerprint_metadata m
LEFT JOIN segmentation_config sc ON m.id = sc.metadata_id
LEFT JOIN segments s ON m.id = s.metadata_id
LEFT JOIN fingerprint_index fi ON m.id = fi.metadata_id
GROUP BY m.id, m.filename, m.duration_ms, m.created_at, sc.enabled, sc.num_segments;

-- Function: get_fingerprints_by_hash
-- Fast lookup of fingerprints by hash value
CREATE OR REPLACE FUNCTION get_fingerprints_by_hash(search_hash BIGINT)
RETURNS TABLE (
    filename VARCHAR,
    segment_id INTEGER,
    t1 INTEGER,
    f1 SMALLINT,
    m1 REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        m.filename,
        fi.segment_id,
        fi.t1,
        fi.f1,
        fi.m1
    FROM fingerprint_index fi
    JOIN fingerprint_metadata m ON fi.metadata_id = m.id
    WHERE fi.hash = search_hash;
END;
$$ LANGUAGE plpgsql;

-- Function: cleanup_orphaned_data
-- Utility function to clean up any orphaned data
CREATE OR REPLACE FUNCTION cleanup_orphaned_data()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER := 0;
BEGIN
    -- Delete fingerprints without segments
    DELETE FROM fingerprints
    WHERE segment_id NOT IN (SELECT id FROM segments);
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Grant permissions
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO panako_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO panako_user;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO panako_user;

-- Insert initial test data (optional)
-- Uncomment to add sample data for testing
/*
INSERT INTO fingerprint_metadata (filename, original_path, sample_rate, duration_ms, channels)
VALUES ('test_audio', '/path/to/test.wav', 16000, 5000, 1);
*/

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'Panako database schema created successfully!';
    RAISE NOTICE 'Tables: fingerprint_metadata, segmentation_config, segments, fingerprints, fingerprint_index';
    RAISE NOTICE 'Views: fingerprint_summary';
    RAISE NOTICE 'Functions: get_fingerprints_by_hash, cleanup_orphaned_data';
END $$;
