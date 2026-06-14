use serde_json::Value;

use crate::okx::generate_okx_client_order_id;

pub(in crate::commands::local_api::trading::actions) fn evidence_client_order_id(
    requested: &str,
) -> String {
    let trimmed = requested.trim();
    if !trimmed.is_empty() {
        trimmed.to_string()
    } else {
        generate_okx_client_order_id()
    }
}

pub(in crate::commands::local_api::trading::actions) fn value_text(
    value: &Value,
    key: &str,
) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(item) => Some(item.to_string()),
        _ => None,
    }
}

pub(in crate::commands::local_api::trading::actions) fn parse_optional_f64(
    value: &str,
) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|item| item.is_finite())
}

pub(in crate::commands::local_api::trading::actions::cost_evidence) fn positive_f64(
    value: Option<&Value>,
) -> Option<f64> {
    let parsed = match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }?;
    if parsed.is_finite() && parsed > 0.0 {
        Some(parsed)
    } else {
        None
    }
}

pub(in crate::commands::local_api::trading::actions::cost_evidence) fn positive_i64(
    value: Option<&Value>,
) -> Option<i64> {
    let parsed = match value? {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }?;
    if parsed > 0 {
        Some(parsed)
    } else {
        None
    }
}

pub(in crate::commands::local_api::trading::actions::cost_evidence) fn now_ms() -> Option<i64> {
    Some(chrono::Utc::now().timestamp_millis())
}
