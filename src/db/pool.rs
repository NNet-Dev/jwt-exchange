//! SQLite connection pool management.
//!
//! Creates and configures the database pool with WAL mode, foreign keys,
//! and auto-creates parent directories. Provides graceful shutdown via
//! WAL checkpoint and connection close.

use anyhow::{Context, Result};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tracing::info;

pub async fn create_pool(db_path: &str) -> Result<SqlitePool> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).context("failed to create database directory")?;
    }

    // F17: Use WAL mode for better concurrency and crash resilience.
    // NOTE: sqlx does not support journal_mode in the connection URL.
    // We must run PRAGMA journal_mode=WAL after connecting.
    let database_url = format!("sqlite:{db_path}?mode=rwc");
    info!(db_path, "Initializing SQLite database (WAL mode)");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to SQLite database")?;

    // Enable WAL mode via PRAGMA (must be done after pool creation)
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .context("failed to set WAL journal mode")?;

    run_migrations(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    info!("Running database migrations");
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("failed to run migrations")?;
    info!("Database migrations complete");
    Ok(())
}

/// Ping the database with a lightweight SELECT 1 to verify connectivity.
pub async fn check_connection(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

/// F17: Checkpoint WAL and truncate on graceful shutdown.
/// Call this before the process exits to ensure all WAL data
/// is flushed to the main database file.
pub async fn checkpoint_and_close(pool: &SqlitePool) {
    // WAL checkpoint: TRUNCATE mode flushes all WAL data to main DB.
    // NOTE: PRAGMA queries must stay as sqlx::query() — query!() does not support pragmas.
    match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
    {
        Ok(result) => {
            let rows_modified = result.rows_affected();
            info!(rows_modified, "SQLite WAL checkpoint complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "SQLite WAL checkpoint failed");
        }
    }

    // Close the pool cleanly
    pool.close().await;
    info!("SQLite pool closed");
}
