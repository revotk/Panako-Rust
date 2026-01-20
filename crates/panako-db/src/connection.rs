//! Database connection management

use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

pub type DbPool = Pool;

/// Create a PostgreSQL connection pool
pub fn create_pool(
    host: &str,
    port: u16,
    database: &str,
    user: &str,
    password: &str,
    _max_connections: u32,
) -> anyhow::Result<DbPool> {
    let mut cfg = Config::new();
    cfg.host = Some(host.to_string());
    cfg.port = Some(port);
    cfg.dbname = Some(database.to_string());
    cfg.user = Some(user.to_string());
    cfg.password = Some(password.to_string());
    
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    
    Ok(pool)
}

/// Test database connection
pub async fn test_connection(pool: &DbPool) -> anyhow::Result<()> {
    let client = pool.get().await?;
    let row = client.query_one("SELECT 1 as test", &[]).await?;
    let test: i32 = row.get(0);
    
    if test == 1 {
        Ok(())
    } else {
        anyhow::bail!("Database connection test failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_create_pool() {
        let pool = create_pool(
            "localhost",
            5432,
            "panako",
            "panako_user",
            "panako_pass",
            10,
        )
        .unwrap();
        assert!(test_connection(&pool).await.is_ok());
    }
}
