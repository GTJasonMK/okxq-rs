use serde_json::Value;

pub(crate) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(item) => Some(item.clone()),
        _ => None,
    }
}

pub(crate) fn json_from_text(value: Option<String>, default_value: Value) -> Value {
    value
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(default_value)
}

pub(crate) fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(crate) fn ts_to_iso(timestamp: Option<i64>) -> Value {
    let Some(ts) = timestamp else {
        return Value::Null;
    };
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts)
        .map(|dt| Value::String(dt.to_rfc3339()))
        .unwrap_or(Value::Null)
}

pub(crate) fn now_unix_seconds() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64 / 1000.0
}

pub(crate) fn generated_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

pub(crate) fn optional_f64_value(value: Option<f64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

pub(crate) fn value_i64_at(value: &Value, key: &str, default: i64) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(default)
}

pub(crate) fn value_string_at(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(value_to_string)
        .filter(|item| !item.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_value_number_helpers_do_not_parse_strings() {
        let value = json!({
            "count": "5",
            "typed_count": 5
        });

        assert_eq!(value_i64_at(&value, "count", 1), 1);
        assert_eq!(value_i64_at(&value, "typed_count", 1), 5);
    }
}
