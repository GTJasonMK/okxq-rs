use sqlx::{Row, SqlitePool};

pub(super) async fn table_exists(pool: &SqlitePool, table: &str) -> anyhow::Result<bool> {
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

pub(super) async fn column_exists(
    pool: &SqlitePool,
    table: &str,
    column: &str,
) -> anyhow::Result<bool> {
    let sql = format!("PRAGMA table_info({})", quote_identifier(table));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| row.try_get::<String, _>("name"))
        .try_fold(false, |found, name| Ok(found || name? == column))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
