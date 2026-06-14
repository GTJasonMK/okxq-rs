use anyhow::{anyhow, Result};
use serde_json::Value;
use sqlx::SqlitePool;

use super::{
    input::{
        alert_from_map, infer_inst_type, normalize_inst_id, normalize_symbol_label, now_text,
        optional_f64, validate_alert, value_bool, value_i64, value_string,
    },
    storage,
};

pub async fn list_alerts(
    pool: &SqlitePool,
    inst_id: Option<&str>,
    inst_type: Option<&str>,
) -> Result<Vec<Value>> {
    let target_inst_id = inst_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_inst_id)
        .transpose()?;
    let target_inst_type = inst_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_uppercase());

    let alerts = storage::list_alerts(pool, target_inst_id, target_inst_type).await?;
    Ok(alerts
        .into_iter()
        .map(|alert| alert.to_value())
        .collect::<Vec<_>>())
}

pub async fn create_alert(pool: &SqlitePool, payload: &Value) -> Result<Value> {
    let obj = payload
        .as_object()
        .ok_or_else(|| anyhow!("价格提醒请求体必须是对象"))?;
    let alert = alert_from_map(obj)?;
    storage::persist_alert(pool, &alert).await?;
    Ok(alert.to_value())
}

pub async fn update_alert(
    pool: &SqlitePool,
    alert_id: &str,
    payload: &Value,
) -> Result<Option<Value>> {
    let Some(mut alert) = storage::get_alert(pool, alert_id).await? else {
        return Ok(None);
    };
    let obj = payload
        .as_object()
        .ok_or_else(|| anyhow!("价格提醒请求体必须是对象"))?;

    if obj.contains_key("inst_id") {
        alert.inst_id = normalize_inst_id(value_string(obj, "inst_id").as_deref().unwrap_or(""))?;
    }
    if obj.contains_key("inst_type") || obj.contains_key("inst_id") {
        alert.inst_type = infer_inst_type(
            &alert.inst_id,
            value_string(obj, "inst_type")
                .as_deref()
                .unwrap_or(&alert.inst_type),
        );
    }
    if obj.contains_key("symbol") {
        alert.symbol = normalize_symbol_label(value_string(obj, "symbol").as_deref().unwrap_or(""));
    }
    if obj.contains_key("alert_type") {
        alert.alert_type = value_string(obj, "alert_type")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
    }
    if obj.contains_key("direction") {
        alert.direction = value_string(obj, "direction")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
    }
    if obj.contains_key("target_price") {
        alert.target_price = optional_f64(obj, "target_price")?;
    }
    if obj.contains_key("change_percent") {
        alert.change_percent = optional_f64(obj, "change_percent")?;
    }
    if obj.contains_key("note") {
        alert.note = value_string(obj, "note")
            .unwrap_or_default()
            .trim()
            .to_string();
    }
    if obj.contains_key("enabled") {
        alert.enabled = value_bool(obj, "enabled", alert.enabled);
    }
    if obj.contains_key("trigger_once") {
        alert.trigger_once = value_bool(obj, "trigger_once", alert.trigger_once);
    }
    if obj.contains_key("cooldown_seconds") {
        alert.cooldown_seconds = value_i64(obj, "cooldown_seconds", alert.cooldown_seconds).max(0);
    }
    if alert.symbol.is_empty() {
        alert.symbol = normalize_symbol_label(&alert.inst_id);
    }
    alert.updated_at = now_text();
    validate_alert(&alert)?;
    storage::persist_alert(pool, &alert).await?;
    Ok(Some(alert.to_value()))
}

pub async fn delete_alert(pool: &SqlitePool, alert_id: &str) -> Result<bool> {
    storage::delete_alert(pool, alert_id).await
}
