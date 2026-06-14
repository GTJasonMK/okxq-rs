use serde_json::Value;

pub fn value_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => Some(item.clone()),
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

pub fn parse_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

pub fn parse_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
}
