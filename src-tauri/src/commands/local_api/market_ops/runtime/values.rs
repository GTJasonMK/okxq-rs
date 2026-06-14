use serde_json::Value;

pub(super) fn json_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|item| usize::try_from(item).ok())
        .or_else(|| value.as_i64().and_then(|item| usize::try_from(item).ok()))
}

pub(super) fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|item| u64::try_from(item).ok()))
}

pub(super) fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
}

pub(super) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

pub(super) fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

pub(super) fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_number_helpers_do_not_parse_strings() {
        assert_eq!(json_usize(&json!(12)), Some(12));
        assert_eq!(json_u64(&json!(12)), Some(12));
        assert_eq!(json_i64(&json!(12)), Some(12));

        assert_eq!(json_usize(&json!("12")), None);
        assert_eq!(json_u64(&json!("12")), None);
        assert_eq!(json_i64(&json!("12")), None);
    }

    #[test]
    fn json_unsigned_helpers_reject_negative_numbers() {
        assert_eq!(json_usize(&json!(-1)), None);
        assert_eq!(json_u64(&json!(-1)), None);
        assert_eq!(json_i64(&json!(-1)), Some(-1));
    }
}
