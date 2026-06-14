use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use super::super::*;

pub(in crate::commands::local_api) async fn scanner_results(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let profile_id = param_string(req, "profile_id", "");
    let limit = param_i64(req, "limit", 50).clamp(1, 200);
    let mut sql = "SELECT * FROM scanner_results".to_string();
    if !profile_id.is_empty() {
        sql.push_str(" WHERE profile_id = ?");
    }
    sql.push_str(" ORDER BY scan_time DESC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if !profile_id.is_empty() {
        query = query.bind(profile_id);
    }
    query = query.bind(limit);
    let data = query
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(scanner_result_row)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(code_ok(Value::Array(data)))
}

fn scanner_result_row(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "profile_id": row.try_get::<String, _>("profile_id")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "inst_type": row.try_get::<String, _>("inst_type")?,
        "timeframe": row.try_get::<String, _>("timeframe")?,
        "matched_conditions": json_array_column(&row, "matched_conditions_json")?,
        "indicator_values": json_object_column(&row, "indicator_values_json")?,
        "price": row.try_get::<f64, _>("price")?,
        "scan_time": row.try_get::<String, _>("scan_time")?
    }))
}

fn json_array_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    let text = row.try_get::<String, _>(column)?;
    let value = serde_json::from_str::<Value>(&text)?;
    match value {
        Value::Array(_) => Ok(value),
        _ => Err(AppError::Runtime(format!(
            "scanner result {column} 不是 JSON 数组"
        ))),
    }
}

fn json_object_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    let text = row.try_get::<String, _>(column)?;
    let value = serde_json::from_str::<Value>(&text)?;
    match value {
        Value::Object(_) => Ok(value),
        _ => Err(AppError::Runtime(format!(
            "scanner result {column} 不是 JSON 对象"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::Row;

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }

    #[tokio::test]
    async fn scanner_results_recent_query_uses_global_recent_index() {
        let db_path = temp_db_path("scanner_results_recent_query_plan");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        let plan = sqlx::query(
            r#"
            EXPLAIN QUERY PLAN
            SELECT * FROM scanner_results
            ORDER BY scan_time DESC
            LIMIT ?
            "#,
        )
        .bind(200_i64)
        .fetch_all(&pool)
        .await
        .expect("explain scanner results query")
        .into_iter()
        .map(|row| row.try_get::<String, _>("detail"))
        .collect::<Result<Vec<_>, _>>()
        .expect("query plan detail rows")
        .join("\n");

        assert!(
            plan.contains("idx_scanner_results_recent"),
            "expected scanner results query to use idx_scanner_results_recent, got:\n{plan}"
        );
        assert!(
            !plan.contains("USE TEMP B-TREE"),
            "scanner results query should not sort through a temp b-tree, got:\n{plan}"
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn scanner_result_row_rejects_invalid_indicator_values_json() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              1 AS id,
              'profile_a' AS profile_id,
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              '1H' AS timeframe,
              '[]' AS matched_conditions_json,
              'not-json' AS indicator_values_json,
              100.0 AS price,
              '2026-01-01T00:00:00Z' AS scan_time
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("scanner result row");

        assert!(super::scanner_result_row(row).is_err());
    }
}
