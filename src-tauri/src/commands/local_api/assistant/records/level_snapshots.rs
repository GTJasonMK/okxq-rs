use super::super::*;
use super::rows::assistant_level_snapshot_row_to_json;

pub(crate) async fn assistant_level_snapshots(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 30).clamp(1, 200);
    let rows =
        sqlx::query("SELECT * FROM assistant_level_snapshots ORDER BY created_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&state.db)
            .await?;
    Ok(code_ok(Value::Array(
        rows.into_iter()
            .map(assistant_level_snapshot_row_to_json)
            .collect::<AppResult<Vec<_>>>()?,
    )))
}

pub(crate) async fn create_assistant_level_snapshot(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let snapshot_id = generated_id("lvl");
    let now = now_text();
    let mode = request_trading_mode(state, req).await?;
    sqlx::query(
        r#"
        INSERT INTO assistant_level_snapshots (
          id, session_id, inst_id, mode, timeframes_json, supports_json, resistances_json,
          invalidation_levels_json, chart_annotations_json, summary_json, metadata_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&snapshot_id)
    .bind(request_string(req, "session_id", ""))
    .bind(request_string(req, "inst_id", ""))
    .bind(mode)
    .bind(serde_json::to_string(
        req.body.get("timeframes").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("supports").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("resistances").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("invalidation_levels").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("chart_annotations").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("summary").unwrap_or(&json!({})),
    )?)
    .bind(serde_json::to_string(
        req.body.get("metadata").unwrap_or(&json!({})),
    )?)
    .bind(&now)
    .execute(&state.db)
    .await?;
    assistant_level_snapshot(state, &snapshot_id).await
}

pub(crate) async fn assistant_level_snapshot(
    state: &AppState,
    snapshot_id: &str,
) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM assistant_level_snapshots WHERE id = ?")
        .bind(snapshot_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(code_ok(match row {
        Some(row) => assistant_level_snapshot_row_to_json(row)?,
        None => Value::Null,
    }))
}
