use std::{path::Path, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

mod migrations;
mod schema;
mod sqlite_meta;

pub async fn connect_and_migrate(db_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    tracing::info!(db_path = %db_path.display(), "opening sqlite database");
    let connect_options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(30))
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(30))
        .connect_with(connect_options)
        .await?;

    // WAL is persistent for a SQLite database file; set it once at startup before
    // the app begins realtime writes and diagnostic reads.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout=30000")
        .execute(&pool)
        .await?;
    migrations::run(&pool).await?;
    tracing::info!(db_path = %db_path.display(), "sqlite database ready");
    Ok(pool)
}

pub async fn db_size_bytes(db_path: &Path) -> u64 {
    tokio::fs::metadata(db_path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}
