use serde_json::Value;

pub(in crate::realtime) fn value_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => Some(item.clone()),
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

pub(in crate::realtime) fn value_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }
}

pub(in crate::realtime) fn parse_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }
}

pub(in crate::realtime) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
