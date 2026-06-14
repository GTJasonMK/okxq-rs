use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use super::{super::*, symbols::resolve_scanner_inst_type};

pub(in crate::commands::local_api) async fn scanner_profiles(state: &AppState) -> AppResult<Value> {
    let rows = sqlx::query("SELECT * FROM scanner_profiles ORDER BY updated_at DESC")
        .fetch_all(&state.db)
        .await?;
    let profiles = rows
        .into_iter()
        .map(scanner_profile_row)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(code_ok(Value::Array(profiles)))
}

pub(in crate::commands::local_api) async fn create_scanner_profile(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let profile_id = req
        .body
        .get("profile_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("sp_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]));
    save_scanner_profile(state, &profile_id, req, None).await?;
    let profile = fetch_scanner_profile(&state.db, &profile_id)
        .await?
        .ok_or_else(|| AppError::Runtime("新建扫描方案后无法读取记录".to_string()))?;
    Ok(code_ok(profile))
}

pub(in crate::commands::local_api) async fn update_scanner_profile(
    state: &AppState,
    profile_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let existing = fetch_scanner_profile(&state.db, profile_id).await?;
    if existing.is_none() {
        return Err(AppError::Validation("扫描方案不存在".to_string()));
    }
    let created_at = existing
        .as_ref()
        .and_then(|value| value.get("created_at"))
        .and_then(Value::as_str)
        .map(str::to_string);
    save_scanner_profile(state, profile_id, req, created_at).await?;
    let profile = fetch_scanner_profile(&state.db, profile_id)
        .await?
        .ok_or_else(|| AppError::Runtime("更新扫描方案后无法读取记录".to_string()))?;
    Ok(code_ok(profile))
}

async fn save_scanner_profile(
    state: &AppState,
    profile_id: &str,
    req: &LocalApiRequest,
    created_at: Option<String>,
) -> AppResult<()> {
    let now = now_text();
    let name = body_string(req, "name", "未命名扫描");
    let conditions = body_array(req, "conditions");
    let logic = body_string(req, "logic", "and");
    let symbols = body_array(req, "symbols");
    let timeframe = body_string(req, "timeframe", "1H");
    let requested_inst_type = body_string(req, "inst_type", "");
    let inst_type = resolve_scanner_inst_type(state, &requested_inst_type).await?;
    let enabled = req
        .body
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let interval_seconds = req
        .body
        .get("interval_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(300)
        .clamp(60, 86_400);
    let created_at = created_at.unwrap_or_else(|| now.clone());

    sqlx::query(
        r#"
        INSERT INTO scanner_profiles (
          profile_id, name, conditions_json, logic, symbols_json,
          timeframe, inst_type, enabled, interval_seconds, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(profile_id) DO UPDATE SET
          name = excluded.name,
          conditions_json = excluded.conditions_json,
          logic = excluded.logic,
          symbols_json = excluded.symbols_json,
          timeframe = excluded.timeframe,
          inst_type = excluded.inst_type,
          enabled = excluded.enabled,
          interval_seconds = excluded.interval_seconds,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(profile_id)
    .bind(name)
    .bind(serde_json::to_string(&conditions)?)
    .bind(logic)
    .bind(serde_json::to_string(&symbols)?)
    .bind(timeframe)
    .bind(inst_type)
    .bind(if enabled { 1 } else { 0 })
    .bind(interval_seconds)
    .bind(created_at)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

pub(in crate::commands::local_api) async fn delete_scanner_profile(
    state: &AppState,
    profile_id: &str,
) -> AppResult<Value> {
    let deleted = sqlx::query("DELETE FROM scanner_profiles WHERE profile_id = ?")
        .bind(profile_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM scanner_results WHERE profile_id = ?")
        .bind(profile_id)
        .execute(&state.db)
        .await?;
    if deleted == 0 {
        return Err(AppError::Validation("扫描方案不存在".to_string()));
    }
    Ok(code_ok(json!({"deleted": true})))
}

pub(super) async fn fetch_scanner_profile(
    pool: &SqlitePool,
    profile_id: &str,
) -> AppResult<Option<Value>> {
    sqlx::query("SELECT * FROM scanner_profiles WHERE profile_id = ?")
        .bind(profile_id)
        .fetch_optional(pool)
        .await?
        .map(scanner_profile_row)
        .transpose()
}

fn scanner_profile_row(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "profile_id": row.try_get::<String, _>("profile_id")?,
        "name": row.try_get::<String, _>("name")?,
        "conditions": json_array_column(&row, "conditions_json")?,
        "logic": row.try_get::<String, _>("logic")?,
        "symbols": json_array_column(&row, "symbols_json")?,
        "timeframe": row.try_get::<String, _>("timeframe")?,
        "inst_type": row.try_get::<String, _>("inst_type")?,
        "enabled": row.try_get::<i64, _>("enabled")? != 0,
        "interval_seconds": row.try_get::<i64, _>("interval_seconds")?,
        "created_at": row.try_get::<String, _>("created_at")?,
        "updated_at": row.try_get::<String, _>("updated_at")?
    }))
}

fn json_array_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    let text = row.try_get::<String, _>(column)?;
    let value = serde_json::from_str::<Value>(&text)?;
    match value {
        Value::Array(_) => Ok(value),
        _ => Err(AppError::Runtime(format!(
            "scanner profile {column} 不是 JSON 数组"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    #[tokio::test]
    async fn scanner_profile_row_rejects_invalid_conditions_json() {
        let pool = test_pool().await;
        let row = sqlx::query(
            r#"
            SELECT
              'profile_a' AS profile_id,
              'Profile A' AS name,
              'not-json' AS conditions_json,
              'and' AS logic,
              '[]' AS symbols_json,
              '1H' AS timeframe,
              'SWAP' AS inst_type,
              1 AS enabled,
              300 AS interval_seconds,
              '2026-01-01T00:00:00Z' AS created_at,
              '2026-01-01T00:00:00Z' AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("profile row");

        assert!(scanner_profile_row(row).is_err());
    }
}
