use super::super::*;
use super::rows::assistant_order_draft_row_to_json;

pub(crate) async fn assistant_order_drafts(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 30).clamp(1, 200);
    let rows = sqlx::query("SELECT * FROM assistant_order_drafts ORDER BY created_at DESC LIMIT ?")
        .bind(limit)
        .fetch_all(&state.db)
        .await?;
    Ok(code_ok(Value::Array(
        rows.into_iter()
            .map(assistant_order_draft_row_to_json)
            .collect::<AppResult<Vec<_>>>()?,
    )))
}

pub(crate) async fn assistant_order_draft(state: &AppState, draft_id: &str) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM assistant_order_drafts WHERE id = ?")
        .bind(draft_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(code_ok(match row {
        Some(row) => assistant_order_draft_row_to_json(row)?,
        None => Value::Null,
    }))
}

pub(crate) async fn create_assistant_order_draft(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let draft_id = generated_id("draft");
    let now = now_text();
    let mode = request_trading_mode(state, req).await?;
    sqlx::query(
        "INSERT INTO assistant_order_drafts (id, session_id, inst_id, mode, side, order_type, size, price, status, risk_json, plan_json, annotations_json, metadata_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&draft_id)
    .bind(request_string(req, "session_id", ""))
    .bind(request_string(req, "inst_id", ""))
    .bind(mode)
    .bind(request_string(req, "side", ""))
    .bind(request_string(req, "order_type", ""))
    .bind(request_string(req, "size", ""))
    .bind(request_string(req, "price", ""))
    .bind(serde_json::to_string(
        req.body.get("risk").unwrap_or(&json!({})),
    )?)
    .bind(serde_json::to_string(
        req.body.get("plan").unwrap_or(&json!({})),
    )?)
    .bind(serde_json::to_string(
        req.body.get("annotations").unwrap_or(&json!([])),
    )?)
    .bind(serde_json::to_string(
        req.body.get("metadata").unwrap_or(&json!({})),
    )?)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;
    assistant_order_draft(state, &draft_id).await
}

pub(crate) async fn confirm_assistant_order_draft(
    state: &AppState,
    draft_id: &str,
) -> AppResult<Value> {
    let now = now_text();
    let affected = sqlx::query(
        "UPDATE assistant_order_drafts SET status = 'confirmed', updated_at = ? WHERE id = ? AND status = 'draft'",
    )
    .bind(&now)
    .bind(draft_id)
    .execute(&state.db)
    .await?
    .rows_affected();
    if affected == 0 {
        return Ok(code_ok(Value::Null).with_message("草案不存在或非 draft 状态，无法确认"));
    }
    assistant_order_draft(state, draft_id).await
}
