use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, now_text, LocalApiRequest},
    error::{AppError, AppResult},
};

use super::super::rows::fetch_journal_entry;
use super::{query::deleted_entry_payload, tag_usage::increment_journal_tag_usage};

pub(crate) async fn create_journal_entry(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let entry_id = req
        .body
        .get("entry_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("je_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]));
    upsert_journal_entry(state, &entry_id, req, true).await?;
    let entry = fetch_journal_entry(&state.db, &entry_id).await?;
    Ok(code_ok(entry.ok_or_else(|| {
        AppError::Runtime(format!("创建日志条目后未读回记录 {entry_id}"))
    })?))
}

pub(crate) async fn update_journal_entry(
    state: &AppState,
    entry_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let existing = fetch_journal_entry(&state.db, entry_id).await?;
    if existing.is_none() {
        return Err(AppError::Validation("日志条目不存在".to_string()));
    }
    upsert_journal_entry(state, entry_id, req, false).await?;
    Ok(code_ok(
        fetch_journal_entry(&state.db, entry_id)
            .await?
            .ok_or_else(|| AppError::Runtime(format!("更新日志条目后未读回记录 {entry_id}")))?,
    ))
}

pub(crate) async fn delete_journal_entry(state: &AppState, entry_id: &str) -> AppResult<Value> {
    let deleted = sqlx::query("DELETE FROM journal_entries WHERE entry_id = ?")
        .bind(entry_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    deleted_entry_payload(deleted).await
}

async fn upsert_journal_entry(
    state: &AppState,
    entry_id: &str,
    req: &LocalApiRequest,
    create: bool,
) -> AppResult<()> {
    let now = now_text();
    let existing = if create {
        None
    } else {
        fetch_journal_entry(&state.db, entry_id).await?
    };
    let read_existing = |key: &str, default_value: Value| {
        existing
            .as_ref()
            .and_then(|value| value.get(key).cloned())
            .unwrap_or(default_value)
    };
    let get_string = |key: &str, default_value: &str| {
        req.body
            .get(key)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                read_existing(key, Value::String(default_value.to_string()))
                    .as_str()
                    .unwrap_or(default_value)
                    .to_string()
            })
    };
    let get_i64 = |key: &str, default_value: i64| {
        req.body
            .get(key)
            .and_then(Value::as_i64)
            .unwrap_or_else(|| {
                read_existing(key, Value::Number(default_value.into()))
                    .as_i64()
                    .unwrap_or(default_value)
            })
    };
    let get_f64 = |key: &str, default_value: f64| {
        req.body
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or_else(|| {
                read_existing(key, json!(default_value))
                    .as_f64()
                    .unwrap_or(default_value)
            })
    };
    let get_array = |key: &str| {
        req.body
            .get(key)
            .filter(|value| value.is_array())
            .cloned()
            .unwrap_or_else(|| read_existing(key, json!([])))
    };
    let get_object = |key: &str| {
        req.body
            .get(key)
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| read_existing(key, json!({})))
    };

    let title = get_string("title", "");
    let content = get_string("content", "");
    let mode = get_string("mode", "simulated");
    let inst_id = get_string("inst_id", "");
    let inst_type = get_string("inst_type", "SPOT");
    let strategy_id = get_string("strategy_id", "");
    let strategy_name = get_string("strategy_name", "");
    let rating = get_i64("rating", 0);
    let emotion = get_string("emotion", "");
    let pnl_snapshot = get_f64("pnl_snapshot", 0.0);
    let trade_ids = get_array("trade_ids");
    let order_ids = get_array("order_ids");
    let tags = get_array("tags");
    let screenshots = get_array("screenshots");
    let metadata = get_object("metadata");
    let created_at = if create {
        req.body
            .get("created_at")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| now.clone())
    } else {
        read_existing("created_at", Value::String(now.clone()))
            .as_str()
            .unwrap_or(&now)
            .to_string()
    };

    sqlx::query(
        r#"
        INSERT INTO journal_entries (
          entry_id, title, content, mode, inst_id, inst_type,
          trade_ids_json, order_ids_json, tags_json,
          strategy_id, strategy_name, rating, emotion,
          screenshots_json, pnl_snapshot, metadata_json,
          created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(entry_id) DO UPDATE SET
          title = excluded.title,
          content = excluded.content,
          mode = excluded.mode,
          inst_id = excluded.inst_id,
          inst_type = excluded.inst_type,
          trade_ids_json = excluded.trade_ids_json,
          order_ids_json = excluded.order_ids_json,
          tags_json = excluded.tags_json,
          strategy_id = excluded.strategy_id,
          strategy_name = excluded.strategy_name,
          rating = excluded.rating,
          emotion = excluded.emotion,
          screenshots_json = excluded.screenshots_json,
          pnl_snapshot = excluded.pnl_snapshot,
          metadata_json = excluded.metadata_json,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(entry_id)
    .bind(title)
    .bind(content)
    .bind(mode)
    .bind(inst_id)
    .bind(inst_type)
    .bind(serde_json::to_string(&trade_ids)?)
    .bind(serde_json::to_string(&order_ids)?)
    .bind(serde_json::to_string(&tags)?)
    .bind(strategy_id)
    .bind(strategy_name)
    .bind(rating)
    .bind(emotion)
    .bind(serde_json::to_string(&screenshots)?)
    .bind(pnl_snapshot)
    .bind(serde_json::to_string(&metadata)?)
    .bind(created_at)
    .bind(now)
    .execute(&state.db)
    .await?;

    increment_journal_tag_usage(state, &tags).await
}
