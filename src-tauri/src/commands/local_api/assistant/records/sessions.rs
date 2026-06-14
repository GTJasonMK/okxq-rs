use super::super::*;
use super::rows::{
    assistant_level_snapshot_row_to_json, assistant_message_row_to_json,
    assistant_order_draft_row_to_json, assistant_session_row_to_json, assistant_step_row_to_json,
};

pub(in crate::commands::local_api::assistant) async fn fetch_assistant_session(
    state: &AppState,
    session_id: &str,
) -> AppResult<Option<Value>> {
    let row = sqlx::query("SELECT * FROM assistant_sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?;
    row.map(assistant_session_row_to_json).transpose()
}

pub(in crate::commands::local_api::assistant) async fn create_assistant_session_record(
    state: &AppState,
    title: &str,
    inst_id: &str,
    inst_type: &str,
    mode: &str,
    metadata: Value,
) -> AppResult<Value> {
    let session_id = generated_id("asst");
    let now = now_text();
    let mut metadata_map = metadata.as_object().cloned().unwrap_or_default();
    metadata_map.insert("inst_id".to_string(), Value::String(inst_id.to_string()));
    metadata_map.insert(
        "inst_type".to_string(),
        Value::String(inst_type.to_string()),
    );
    metadata_map.insert("mode".to_string(), Value::String(mode.to_string()));
    sqlx::query(
        "INSERT INTO assistant_sessions (id, kind, title, metadata_json, created_at, updated_at) VALUES (?, 'agent', ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(title)
    .bind(serde_json::to_string(&Value::Object(metadata_map))?)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;
    fetch_assistant_session(state, &session_id)
        .await?
        .ok_or_else(|| AppError::Runtime(format!("创建助手会话后未读回记录 {session_id}")))
}

pub(in crate::commands::local_api::assistant) async fn append_assistant_message(
    state: &AppState,
    session_id: &str,
    role: &str,
    content: &str,
    metadata: Value,
) -> AppResult<Value> {
    let message_id = generated_id("msg");
    let now = now_text();
    sqlx::query(
        "INSERT INTO assistant_messages (id, session_id, role, content, metadata_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(serde_json::to_string(&metadata)?)
    .bind(&now)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE assistant_sessions SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(session_id)
        .execute(&state.db)
        .await?;
    let row = sqlx::query("SELECT * FROM assistant_messages WHERE id = ?")
        .bind(&message_id)
        .fetch_one(&state.db)
        .await?;
    assistant_message_row_to_json(row)
}

pub(in crate::commands::local_api::assistant) async fn assistant_session_detail_value(
    state: &AppState,
    session_id: &str,
) -> AppResult<Value> {
    let Some(session) = fetch_assistant_session(state, session_id).await? else {
        return Err(AppError::Validation(format!("未找到会话 {session_id}")));
    };
    let messages = sqlx::query(
        "SELECT * FROM assistant_messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(assistant_message_row_to_json)
    .collect::<AppResult<Vec<_>>>()?;
    let steps =
        sqlx::query("SELECT * FROM assistant_steps WHERE session_id = ? ORDER BY created_at ASC")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .map(assistant_step_row_to_json)
            .collect::<AppResult<Vec<_>>>()?;
    let order_drafts = sqlx::query(
        "SELECT * FROM assistant_order_drafts WHERE session_id = ? ORDER BY created_at DESC LIMIT 50",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(assistant_order_draft_row_to_json)
    .collect::<AppResult<Vec<_>>>()?;
    let level_snapshots = sqlx::query(
        "SELECT * FROM assistant_level_snapshots WHERE session_id = ? ORDER BY created_at DESC LIMIT 50",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(assistant_level_snapshot_row_to_json)
    .collect::<AppResult<Vec<_>>>()?;
    Ok(json!({
        "session": session,
        "messages": messages,
        "steps": steps,
        "order_drafts": order_drafts,
        "level_snapshots": level_snapshots
    }))
}

pub(crate) async fn assistant_agent_sessions(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 30).clamp(1, 200);
    let rows = sqlx::query(
        "SELECT * FROM assistant_sessions WHERE kind = 'agent' ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(code_ok(Value::Array(
        rows.into_iter()
            .map(assistant_session_row_to_json)
            .collect::<AppResult<Vec<_>>>()?,
    )))
}

pub(crate) async fn create_assistant_agent_session(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_string(req, "inst_id", "");
    let inst_type = request_string(req, "inst_type", "SPOT");
    let mode = request_trading_mode(state, req).await?;
    let title = request_string(req, "title", "AI 分析会话");
    let metadata = req
        .body
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let session =
        create_assistant_session_record(state, &title, &inst_id, &inst_type, &mode, metadata)
            .await?;
    Ok(code_ok(session))
}

pub(crate) async fn assistant_agent_session_detail(
    state: &AppState,
    session_id: &str,
) -> AppResult<Value> {
    Ok(code_ok(
        assistant_session_detail_value(state, session_id).await?,
    ))
}
