use serde_json::Value;

pub(in crate::commands::local_api::trading) fn optional_param(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(super) fn value_string(value: &Value, key: &str, default: impl Into<String>) -> String {
    let default = default.into();
    match value.get(key) {
        Some(Value::String(item)) => item.clone(),
        Some(Value::Number(item)) => item.to_string(),
        Some(Value::Bool(item)) => item.to_string(),
        _ => default,
    }
}

pub(super) fn parse_value_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }?;
    parsed.is_finite().then_some(parsed)
}

pub(super) fn positive_i64_opt(value: &Value, key: &str) -> Option<i64> {
    let parsed = match value.get(key)? {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }?;
    (parsed > 0).then_some(parsed)
}
