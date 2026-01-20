# Panako PostgreSQL Setup

## Quick Start with Docker

### 1. Start PostgreSQL
```bash
docker-compose up -d
```

### 2. Verify Database is Running
```bash
docker-compose ps
```

### 3. Connect to Database
```bash
docker exec -it panako-postgres psql -U panako_user -d panako
```

### 4. Stop Database
```bash
docker-compose down
```

### 5. Stop and Remove Data
```bash
docker-compose down -v
```

## Database Configuration

- **Host**: localhost
- **Port**: 5432
- **Database**: panako
- **User**: panako_user
- **Password**: panako_pass

## Connection String

```
postgresql://panako_user:panako_pass@localhost:5432/panako
```

## Schema Overview

### Tables

1. **fingerprint_metadata** - Audio file metadata
2. **segmentation_config** - Segmentation settings
3. **segments** - Individual audio segments
4. **fingerprints** - Fingerprint data (JSONB)
5. **fingerprint_index** - Optimized hash index

### Views

- **fingerprint_summary** - Statistics and counts

### Functions

- **get_fingerprints_by_hash(hash)** - Fast hash lookup
- **cleanup_orphaned_data()** - Maintenance utility

## Useful SQL Queries

### View all fingerprints
```sql
SELECT * FROM fingerprint_summary;
```

### Search by hash
```sql
SELECT * FROM get_fingerprints_by_hash(12345678901234);
```

### Get fingerprint count
```sql
SELECT filename, total_fingerprints 
FROM fingerprint_summary 
ORDER BY total_fingerprints DESC;
```

### Check database size
```sql
SELECT pg_size_pretty(pg_database_size('panako'));
```

## Maintenance

### Cleanup orphaned data
```sql
SELECT cleanup_orphaned_data();
```

### Vacuum database
```sql
VACUUM ANALYZE;
```

## Troubleshooting

### Check if PostgreSQL is ready
```bash
docker exec panako-postgres pg_isready -U panako_user -d panako
```

### View PostgreSQL logs
```bash
docker-compose logs postgres
```

### Reset database
```bash
docker-compose down -v
docker-compose up -d
```
