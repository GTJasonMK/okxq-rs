use serde_json::Value;

pub(super) fn json_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(item)) => item.clone(),
        Some(Value::Number(item)) => item.to_string(),
        Some(Value::Bool(item)) => item.to_string(),
        _ => String::new(),
    }
}

pub(super) fn json_positive_f64(value: &Value, key: &str) -> Option<f64> {
    json_finite_f64(value, key).filter(|item| *item > 0.0)
}

pub(super) fn json_finite_f64(value: &Value, key: &str) -> Option<f64> {
    let parsed = match value.get(key)? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    parsed.is_finite().then_some(parsed)
}
