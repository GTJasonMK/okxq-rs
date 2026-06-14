use super::super::*;
use sqlx::{sqlite::SqliteRow, Row};

pub(super) fn assistant_session_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let metadata = json_column(&row, "metadata_json")?;
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "session_id": id,
        "kind": row.try_get::<String, _>("kind")?,
        "title": row.try_get::<String, _>("title")?,
        "mode": value_string_at(&metadata, "mode", "simulated"),
        "inst_id": value_string_at(&metadata, "inst_id", ""),
        "inst_type": value_string_at(&metadata, "inst_type", "SPOT"),
        "metadata": metadata,
        "created_at": row.try_get::<String, _>("created_at")?,
        "updated_at": row.try_get::<String, _>("updated_at")?
    }))
}

pub(super) fn assistant_message_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "message_id": id,
        "session_id": row.try_get::<String, _>("session_id")?,
        "role": row.try_get::<String, _>("role")?,
        "content": row.try_get::<String, _>("content")?,
        "metadata": json_column(&row, "metadata_json")?,
        "created_at": row.try_get::<String, _>("created_at")?
    }))
}

pub(super) fn assistant_step_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "step_id": id,
        "session_id": row.try_get::<String, _>("session_id")?,
        "step_type": row.try_get::<String, _>("step_type")?,
        "title": row.try_get::<String, _>("title")?,
        "input": json_column(&row, "input_json")?,
        "output": json_column(&row, "output_json")?,
        "status": "completed",
        "created_at": row.try_get::<String, _>("created_at")?
    }))
}

pub(super) fn assistant_order_draft_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "draft_id": id,
        "session_id": row.try_get::<String, _>("session_id")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "mode": row.try_get::<String, _>("mode")?,
        "side": row.try_get::<String, _>("side")?,
        "order_type": row.try_get::<String, _>("order_type")?,
        "size": row.try_get::<String, _>("size")?,
        "price": row.try_get::<String, _>("price")?,
        "status": row.try_get::<String, _>("status")?,
        "risk": json_column(&row, "risk_json")?,
        "plan": json_column(&row, "plan_json")?,
        "annotations": json_column(&row, "annotations_json")?,
        "metadata": json_column(&row, "metadata_json")?,
        "created_at": row.try_get::<String, _>("created_at")?,
        "updated_at": row.try_get::<String, _>("updated_at")?
    }))
}

pub(super) fn assistant_level_snapshot_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "snapshot_id": id,
        "session_id": row.try_get::<String, _>("session_id")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "mode": row.try_get::<String, _>("mode")?,
        "timeframes": json_column(&row, "timeframes_json")?,
        "supports": json_column(&row, "supports_json")?,
        "resistances": json_column(&row, "resistances_json")?,
        "invalidation_levels": json_column(&row, "invalidation_levels_json")?,
        "chart_annotations": json_column(&row, "chart_annotations_json")?,
        "summary": json_column(&row, "summary_json")?,
        "metadata": json_column(&row, "metadata_json")?,
        "created_at": row.try_get::<String, _>("created_at")?
    }))
}

pub(super) fn assistant_patrol_run_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let id = row.try_get::<String, _>("id")?;
    Ok(json!({
        "id": id,
        "run_id": id,
        "mode": row.try_get::<String, _>("mode")?,
        "status": row.try_get::<String, _>("status")?,
        "summary": json_column(&row, "summary_json")?,
        "candidates": json_column(&row, "candidates_json")?,
        "result": json_column(&row, "result_json")?,
        "event": json_column(&row, "event_json")?,
        "settings": json_column(&row, "settings_json")?,
        "started_at": row.try_get::<String, _>("started_at")?,
        "finished_at": row.try_get::<Option<String>, _>("finished_at")?
    }))
}

fn json_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    Ok(serde_json::from_str(&row.try_get::<String, _>(column)?)?)
}
