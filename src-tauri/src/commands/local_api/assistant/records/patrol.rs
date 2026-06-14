use super::super::*;
use super::rows::assistant_patrol_run_row_to_json;

fn assistant_patrol_default_settings() -> Value {
    json!({
        "enabled": false,
        "interval_seconds": 300,
        "scan_limit": 24,
        "candidate_limit": 3,
        "inst_type": "SWAP",
        "timeframes": ["1H", "4H"],
        "candles_limit": 240,
        "recent_trade_limit": 40,
        "orderbook_depth": 30,
        "mode": "simulated",
        "min_priority_score": 55,
        "notification_cooldown_seconds": 900
    })
}

pub(crate) fn assistant_patrol_status() -> AppResult<Value> {
    Ok(code_ok(json!({
        "running": false,
        "current_phase": "idle",
        "last_run_started_at": Value::Null,
        "last_run_finished_at": Value::Null,
        "last_run_summary": {},
        "last_error": "",
        "recent_events": [],
        "settings": assistant_patrol_default_settings()
    })))
}

pub(crate) fn assistant_patrol_config() -> AppResult<Value> {
    Ok(code_ok(assistant_patrol_default_settings()))
}

pub(crate) fn assistant_update_patrol_config(req: &LocalApiRequest) -> AppResult<Value> {
    let settings = assistant_patrol_settings_from_request(req);
    let status_settings = settings.clone();
    Ok(code_ok(json!({
        "settings": settings,
        "status": {
            "running": false,
            "current_phase": "idle",
            "settings": status_settings
        }
    })))
}

fn assistant_patrol_settings_from_request(req: &LocalApiRequest) -> Value {
    let mut settings = assistant_patrol_default_settings()
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(body) = req.body.as_object() {
        copy_bool_setting(&mut settings, body, "enabled");
        copy_number_setting(&mut settings, body, "interval_seconds");
        copy_number_setting(&mut settings, body, "scan_limit");
        copy_number_setting(&mut settings, body, "candidate_limit");
        copy_number_setting(&mut settings, body, "candles_limit");
        copy_number_setting(&mut settings, body, "recent_trade_limit");
        copy_number_setting(&mut settings, body, "orderbook_depth");
        copy_number_setting(&mut settings, body, "min_priority_score");
        copy_number_setting(&mut settings, body, "notification_cooldown_seconds");
        copy_string_setting(&mut settings, body, "inst_type");
        copy_string_setting(&mut settings, body, "mode");
        copy_string_array_setting(&mut settings, body, "symbols");
        copy_string_array_setting(&mut settings, body, "timeframes");
    }
    Value::Object(settings)
}

pub(crate) async fn assistant_run_patrol_now(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let run_id = generated_id("patrol");
    let now = now_text();
    let mode = request_trading_mode(state, req).await?;
    let settings = assistant_patrol_settings_from_request(req);
    let summary = json!({"candidate_count": 0, "message": "本地巡检已完成，未发现候选机会"});
    let event = json!({
        "id": generated_id("evt"),
        "run_id": run_id,
        "type": "assistant_patrol_completed",
        "message": "本地巡检已完成",
        "created_at": now
    });
    sqlx::query(
        "INSERT INTO assistant_patrol_runs (id, mode, status, summary_json, candidates_json, result_json, event_json, settings_json, started_at, finished_at) VALUES (?, ?, 'completed', ?, '[]', ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(mode)
    .bind(serde_json::to_string(&summary)?)
    .bind(serde_json::to_string(&json!({"summary": summary, "candidates": []}))?)
    .bind(serde_json::to_string(&event)?)
    .bind(serde_json::to_string(&settings)?)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;
    let row = sqlx::query("SELECT * FROM assistant_patrol_runs WHERE id = ?")
        .bind(&run_id)
        .fetch_one(&state.db)
        .await?;
    Ok(code_ok(assistant_patrol_run_row_to_json(row)?))
}

pub(crate) async fn assistant_patrol_runs(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 30).clamp(1, 200);
    let rows = sqlx::query("SELECT * FROM assistant_patrol_runs ORDER BY started_at DESC LIMIT ?")
        .bind(limit)
        .fetch_all(&state.db)
        .await?;
    Ok(code_ok(Value::Array(
        rows.into_iter()
            .map(assistant_patrol_run_row_to_json)
            .collect::<AppResult<Vec<_>>>()?,
    )))
}

pub(crate) async fn assistant_patrol_run(state: &AppState, run_id: &str) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM assistant_patrol_runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(code_ok(match row {
        Some(row) => assistant_patrol_run_row_to_json(row)?,
        None => Value::Null,
    }))
}

fn copy_bool_setting(
    settings: &mut serde_json::Map<String, Value>,
    body: &serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(Value::Bool(value)) = body.get(key) {
        settings.insert(key.to_string(), Value::Bool(*value));
    }
}

fn copy_number_setting(
    settings: &mut serde_json::Map<String, Value>,
    body: &serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(Value::Number(value)) = body.get(key) {
        settings.insert(key.to_string(), Value::Number(value.clone()));
    }
}

fn copy_string_setting(
    settings: &mut serde_json::Map<String, Value>,
    body: &serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(Value::String(value)) = body.get(key) {
        settings.insert(key.to_string(), Value::String(value.clone()));
    }
}

fn copy_string_array_setting(
    settings: &mut serde_json::Map<String, Value>,
    body: &serde_json::Map<String, Value>,
    key: &str,
) {
    let Some(Value::Array(values)) = body.get(key) else {
        return;
    };
    if values.iter().all(Value::is_string) {
        settings.insert(key.to_string(), Value::Array(values.clone()));
    }
}
