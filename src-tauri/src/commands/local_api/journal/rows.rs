use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::error::{AppError, AppResult};

pub(super) async fn fetch_journal_entry(
    pool: &SqlitePool,
    entry_id: &str,
) -> AppResult<Option<Value>> {
    sqlx::query("SELECT * FROM journal_entries WHERE entry_id = ?")
        .bind(entry_id)
        .fetch_optional(pool)
        .await?
        .map(journal_row_to_json)
        .transpose()
}

pub(super) fn journal_row_to_json(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "entry_id": row.try_get::<String, _>("entry_id")?,
        "title": row.try_get::<String, _>("title")?,
        "content": row.try_get::<String, _>("content")?,
        "mode": row.try_get::<String, _>("mode")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "inst_type": row.try_get::<String, _>("inst_type")?,
        "trade_ids": json_column(&row, "trade_ids_json")?,
        "order_ids": json_column(&row, "order_ids_json")?,
        "tags": json_column(&row, "tags_json")?,
        "strategy_id": row.try_get::<String, _>("strategy_id")?,
        "strategy_name": row.try_get::<String, _>("strategy_name")?,
        "rating": row.try_get::<i64, _>("rating")?,
        "emotion": row.try_get::<String, _>("emotion")?,
        "screenshots": json_column(&row, "screenshots_json")?,
        "pnl_snapshot": row.try_get::<f64, _>("pnl_snapshot")?,
        "metadata": json_column(&row, "metadata_json")?,
        "created_at": row.try_get::<Option<String>, _>("created_at")?,
        "updated_at": row.try_get::<Option<String>, _>("updated_at")?,
    }))
}

fn json_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    let Some(text) = row.try_get::<Option<String>, _>(column)? else {
        return Err(AppError::Runtime(format!(
            "journal entry JSON column `{column}` is NULL"
        )));
    };
    Ok(serde_json::from_str(&text)?)
}
