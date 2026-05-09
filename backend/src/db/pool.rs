use crate::error::{AppError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let max_conn: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    let options = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| AppError::Config(format!("Invalid DATABASE_URL: {}", e)))?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(max_conn)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./src/db/migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Config(format!("Migration failed: {}", e)))?;

    tracing::info!(
        "Database connected and migrations applied (max_connections={})",
        max_conn
    );
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn create_pool_runs_migrations_on_memory_db() {
        unsafe { std::env::set_var("DB_MAX_CONNECTIONS", "1") };
        let pool = create_pool("sqlite::memory:").await.unwrap();
        // The `shows` table exists if migrations ran.
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shows")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 0);
        unsafe { std::env::remove_var("DB_MAX_CONNECTIONS") };
    }

    #[tokio::test]
    #[serial]
    async fn create_pool_uses_default_max_connections_when_env_invalid() {
        unsafe { std::env::set_var("DB_MAX_CONNECTIONS", "not-a-number") };
        // Falls back to default 5 — should still succeed.
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let _: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        unsafe { std::env::remove_var("DB_MAX_CONNECTIONS") };
    }
}
