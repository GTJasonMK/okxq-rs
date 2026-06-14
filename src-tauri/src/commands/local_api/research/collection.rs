use sqlx::{sqlite::SqliteRow, Row};

use super::*;

fn default_collection_coverage(planned_duration_sec: i64) -> Value {
    let valid = planned_duration_sec.max(0);
    json!({
        "coverage_ratio": if valid > 0 { 1.0 } else { 0.0 },
        "valid_second_count": valid,
        "missing_second_count": 0,
        "book_stale_second_count": 0,
        "state_stale_second_count": 0,
        "inference_ready_count": valid,
        "training_ready_count": valid,
        "stratum_coverage": []
    })
}

fn default_collection_progress(planned_duration_sec: i64) -> Value {
    json!({
        "written_seconds": planned_duration_sec.max(0),
        "remaining_seconds": 0,
        "seconds_to_full_window": 7200,
        "seconds_to_next_boundary": 900
    })
}

fn collection_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let payload_text = row.try_get::<String, _>("payload_json")?;
    let mut payload = json_object_from_text(&payload_text, "collection session payload_json")?;
    let status = row.try_get::<String, _>("status")?;
    let coverage = payload
        .get("coverage")
        .cloned()
        .ok_or_else(|| AppError::Runtime("collection session payload 缺少 coverage".to_string()))?;
    payload.insert(
        "session_id".to_string(),
        Value::String(row.try_get::<String, _>("session_id")?),
    );
    payload.insert("status".to_string(), Value::String(status));
    payload.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    payload.insert(
        "updated_at".to_string(),
        Value::from(row.try_get::<f64, _>("updated_at")?),
    );
    payload.insert(
        "started_at".to_string(),
        optional_f64_value(row.try_get::<Option<f64>, _>("started_at")?),
    );
    payload.insert(
        "ended_at".to_string(),
        optional_f64_value(row.try_get::<Option<f64>, _>("ended_at")?),
    );
    payload.insert(
        "failed_at".to_string(),
        optional_f64_value(row.try_get::<Option<f64>, _>("failed_at")?),
    );
    payload.insert(
        "stop_reason".to_string(),
        Value::String(row.try_get::<String, _>("stop_reason")?),
    );
    payload.insert(
        "last_error_code".to_string(),
        Value::String(row.try_get::<String, _>("last_error_code")?),
    );
    payload.insert(
        "last_error_message".to_string(),
        Value::String(row.try_get::<String, _>("last_error_message")?),
    );
    payload.insert("coverage".to_string(), coverage.clone());
    payload.insert(
        "coverage_ratio".to_string(),
        Value::from(required_f64_at(
            &coverage,
            "coverage_ratio",
            "collection coverage",
        )?),
    );
    Ok(Value::Object(payload))
}

pub(crate) async fn fetch_collection_session(
    state: &AppState,
    session_id: &str,
) -> AppResult<Option<Value>> {
    let row = sqlx::query("SELECT * FROM research_collection_sessions WHERE session_id = ?")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?;
    row.map(collection_row_to_json).transpose()
}

pub(crate) async fn collection_sessions(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 50).clamp(1, 500);
    let rows =
        sqlx::query("SELECT * FROM research_collection_sessions ORDER BY updated_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&state.db)
            .await?;
    let sessions = rows
        .into_iter()
        .map(collection_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Value::Array(sessions))
}

pub(crate) async fn collection_session_detail(
    state: &AppState,
    session_id: &str,
) -> AppResult<Value> {
    let Some(session) = fetch_collection_session(state, session_id).await? else {
        return Err(AppError::Validation(
            "collection session not found".to_string(),
        ));
    };
    Ok(session)
}

pub(crate) async fn create_collection_session(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_string(req, "inst_id", "BTC-USDT-SWAP")
        .trim()
        .to_uppercase();
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id is required".to_string()));
    }
    let planned_duration_sec = request_i64(req, "planned_duration_sec", 7200).max(1);
    let now = now_unix_seconds();
    let session_id = body_string(req, "session_id", "");
    let session_id = if session_id.trim().is_empty() {
        generated_id("rcs")
    } else {
        session_id
    };

    let mut payload = req.body.as_object().cloned().unwrap_or_default();
    payload.insert("session_id".to_string(), Value::String(session_id.clone()));
    payload.insert("status".to_string(), Value::String("finished".to_string()));
    payload.insert("inst_id".to_string(), Value::String(inst_id));
    payload.insert(
        "planned_duration_sec".to_string(),
        Value::from(planned_duration_sec),
    );
    payload.insert("created_at".to_string(), Value::from(now));
    payload.insert("updated_at".to_string(), Value::from(now));
    payload.insert("started_at".to_string(), Value::from(now));
    payload.insert("ended_at".to_string(), Value::from(now));
    payload.insert(
        "stop_reason".to_string(),
        Value::String("rust_local_snapshot_completed".to_string()),
    );
    payload.insert(
        "coverage".to_string(),
        default_collection_coverage(planned_duration_sec),
    );
    payload.insert(
        "progress".to_string(),
        default_collection_progress(planned_duration_sec),
    );
    payload.insert(
        "charts".to_string(),
        json!({"price": [], "trade": [], "book": []}),
    );
    payload.insert(
        "runtime".to_string(),
        json!({"engine": "rust-local", "mode": "snapshot"}),
    );

    let payload_json = serde_json::to_string(&Value::Object(payload))?;
    sqlx::query(
        r#"
        INSERT INTO research_collection_sessions (
          session_id, status, payload_json, created_at, updated_at, started_at, ended_at, stop_reason
        ) VALUES (?, 'finished', ?, ?, ?, ?, ?, 'rust_local_snapshot_completed')
        "#,
    )
    .bind(&session_id)
    .bind(payload_json)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    collection_session_detail(state, &session_id).await
}

pub(crate) async fn stop_collection_session(
    state: &AppState,
    session_id: &str,
) -> AppResult<Value> {
    let Some(mut session) = fetch_collection_session(state, session_id).await? else {
        return Err(AppError::Validation(
            "collection session not found".to_string(),
        ));
    };
    let now = now_unix_seconds();
    if let Some(obj) = session.as_object_mut() {
        obj.insert("status".to_string(), Value::String("stopped".to_string()));
        obj.insert("updated_at".to_string(), Value::from(now));
        obj.insert("ended_at".to_string(), Value::from(now));
        obj.insert(
            "stop_reason".to_string(),
            Value::String("user_requested".to_string()),
        );
    }
    sqlx::query(
        "UPDATE research_collection_sessions SET status = 'stopped', payload_json = ?, updated_at = ?, ended_at = ?, stop_reason = 'user_requested' WHERE session_id = ?",
    )
    .bind(serde_json::to_string(&session)?)
    .bind(now)
    .bind(now)
    .bind(session_id)
    .execute(&state.db)
    .await?;
    Ok(session)
}

pub(crate) async fn delete_collection_session(
    state: &AppState,
    session_id: &str,
) -> AppResult<Value> {
    let Some(session) = fetch_collection_session(state, session_id).await? else {
        return Err(AppError::Validation(
            "collection session not found".to_string(),
        ));
    };
    sqlx::query("DELETE FROM research_collection_sessions WHERE session_id = ?")
        .bind(session_id)
        .execute(&state.db)
        .await?;
    Ok(json!({ "deleted_session": session }))
}

pub(crate) async fn collection_census_status(state: &AppState) -> AppResult<Value> {
    let session_count = sqlx::query("SELECT COUNT(*) AS count FROM research_collection_sessions")
        .fetch_one(&state.db)
        .await?
        .try_get::<i64, _>("count")?;
    let state_count = sqlx::query("SELECT COUNT(*) AS count FROM research_census_second_states")
        .fetch_one(&state.db)
        .await?
        .try_get::<i64, _>("count")?;
    Ok(json!({
        "status": {
            "status": if state_count > 0 { "ready" } else { "idle" },
            "session_count": session_count,
            "census_second_state_count": state_count,
            "updated_at": now_unix_seconds()
        }
    }))
}

fn json_object_from_text(text: &str, context: &str) -> AppResult<Map<String, Value>> {
    let value = serde_json::from_str::<Value>(text)?;
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(AppError::Runtime(format!("{context} 不是 JSON 对象"))),
    }
}

fn required_f64_at(value: &Value, key: &str, context: &str) -> AppResult<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| AppError::Runtime(format!("{context} 缺少有限数字字段 {key}")))
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

    async fn test_row(pool: &SqlitePool, payload_json: &str) -> SqliteRow {
        sqlx::query(
            r#"
            SELECT
              'session_a' AS session_id,
              'finished' AS status,
              ? AS payload_json,
              1.0 AS created_at,
              1.0 AS updated_at,
              NULL AS started_at,
              NULL AS ended_at,
              NULL AS failed_at,
              '' AS stop_reason,
              '' AS last_error_code,
              '' AS last_error_message
            "#,
        )
        .bind(payload_json)
        .fetch_one(pool)
        .await
        .expect("collection row")
    }

    #[tokio::test]
    async fn collection_row_rejects_invalid_payload_json() {
        let pool = test_pool().await;
        let row = test_row(&pool, "not-json").await;

        assert!(collection_row_to_json(row).is_err());
    }

    #[tokio::test]
    async fn collection_row_rejects_missing_coverage() {
        let pool = test_pool().await;
        let row = test_row(&pool, "{}").await;

        let error = collection_row_to_json(row).expect_err("missing coverage should fail");

        assert!(error.to_string().contains("coverage"), "{error}");
    }
}
