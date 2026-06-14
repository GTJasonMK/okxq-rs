use serde_json::Value;

use crate::app_state::AppState;

use super::{clients::normalize_trading_mode, values::value_to_string};
use crate::commands::local_api::LocalApiRequest;

pub(crate) fn param_string(req: &LocalApiRequest, key: &str, default: &str) -> String {
    req.params
        .get(key)
        .and_then(value_to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn param_bool(req: &LocalApiRequest, key: &str, default: bool) -> bool {
    req.params
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub(crate) fn param_i64(req: &LocalApiRequest, key: &str, default: i64) -> i64 {
    req.params
        .get(key)
        .and_then(Value::as_i64)
        .unwrap_or(default)
}

pub(crate) fn request_string(req: &LocalApiRequest, key: &str, default: &str) -> String {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(value_to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn request_runtime_string(
    req: &LocalApiRequest,
    params: &serde_json::Map<String, Value>,
    key: &str,
    fallback: &str,
) -> String {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .or_else(|| params.get(key))
        .and_then(value_to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(crate) fn request_runtime_f64(
    req: &LocalApiRequest,
    params: &serde_json::Map<String, Value>,
    key: &str,
    fallback: f64,
) -> f64 {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(finite_value_from_json)
        .or_else(|| request_runtime_f64_opt(params, key))
        .unwrap_or(fallback)
}

pub(crate) fn request_runtime_f64_from_params(
    params: &serde_json::Map<String, Value>,
    key: &str,
    fallback: f64,
) -> f64 {
    request_runtime_f64_opt(params, key).unwrap_or(fallback)
}

pub(crate) fn request_runtime_f64_opt(
    params: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<f64> {
    params.get(key).and_then(finite_value_from_json)
}

pub(crate) fn request_runtime_bool(
    params: &serde_json::Map<String, Value>,
    key: &str,
    fallback: bool,
) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

pub(crate) fn finite_value_from_json(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|item| item as f64))
        .or_else(|| value.as_u64().map(|item| item as f64))
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|item| item.is_finite())
}

pub(crate) async fn request_trading_mode(
    state: &AppState,
    req: &LocalApiRequest,
) -> crate::error::AppResult<String> {
    if let Some(mode) = req
        .body
        .get("mode")
        .or_else(|| req.params.get("mode"))
        .and_then(value_to_string)
        .filter(|value| !value.is_empty())
    {
        return normalize_trading_mode(&mode);
    }
    normalize_trading_mode(state.config.read().await.okx.default_mode())
}

pub(crate) fn request_i64(req: &LocalApiRequest, key: &str, default: i64) -> i64 {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(Value::as_i64)
        .unwrap_or(default)
}

pub(crate) fn request_f64(req: &LocalApiRequest, key: &str, default: f64) -> f64 {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

pub(crate) fn request_optional_f64(req: &LocalApiRequest, key: &str) -> Option<f64> {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(Value::as_f64)
}

pub(crate) fn param_f64(req: &LocalApiRequest, key: &str, default: f64) -> f64 {
    req.params
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

pub(crate) fn body_string(req: &LocalApiRequest, key: &str, default: &str) -> String {
    req.body
        .get(key)
        .and_then(value_to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn body_i64(req: &LocalApiRequest, key: &str, default: i64) -> i64 {
    req.body.get(key).and_then(Value::as_i64).unwrap_or(default)
}

pub(crate) fn body_bool(req: &LocalApiRequest, key: &str, default: bool) -> bool {
    req.body
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub(crate) fn body_array(req: &LocalApiRequest, key: &str) -> Vec<Value> {
    req.body
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn request_bool(req: &LocalApiRequest, key: &str, default: bool) -> bool {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub(crate) fn request_string_array(req: &LocalApiRequest, key: &str) -> Vec<String> {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .map(value_to_string_array)
        .unwrap_or_default()
}

fn value_to_string_array(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(value_to_string)
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::*;
    use crate::commands::local_api::helpers::value_string_at;

    fn request(params: Value, body: Value) -> LocalApiRequest {
        LocalApiRequest {
            method: "GET".to_string(),
            path: "/".to_string(),
            params: params.as_object().cloned().unwrap_or_default(),
            body,
        }
    }

    #[test]
    fn scalar_helpers_require_native_json_types() {
        let req = request(
            json!({
                "limit": "25",
                "confidence": "0.95",
                "active": "true"
            }),
            json!({
                "days": "30",
                "ratio": "0.5",
                "enabled": 1,
                "typed_days": 30,
                "typed_ratio": 0.5,
                "typed_enabled": true
            }),
        );

        assert_eq!(param_i64(&req, "limit", 10), 10);
        assert_eq!(param_f64(&req, "confidence", 0.8), 0.8);
        assert!(!param_bool(&req, "active", false));
        assert_eq!(request_i64(&req, "days", 7), 7);
        assert_eq!(request_f64(&req, "ratio", 0.2), 0.2);
        assert!(!request_bool(&req, "enabled", false));
        assert_eq!(body_i64(&req, "days", 7), 7);
        assert!(!body_bool(&req, "enabled", false));

        assert_eq!(request_i64(&req, "typed_days", 7), 30);
        assert_eq!(request_f64(&req, "typed_ratio", 0.2), 0.5);
        assert!(request_bool(&req, "typed_enabled", false));
        assert_eq!(body_i64(&req, "typed_days", 7), 30);
        assert!(body_bool(&req, "typed_enabled", false));
    }

    #[test]
    fn string_helpers_do_not_stringify_non_strings() {
        let req = request(
            Map::from_iter([
                ("symbol".to_string(), Value::String("BTC-USDT".to_string())),
                ("numeric".to_string(), Value::from(42)),
                ("flag".to_string(), Value::Bool(true)),
            ])
            .into(),
            json!({
                "body_symbol": "ETH-USDT",
                "body_numeric": 42,
                "body_flag": false
            }),
        );

        assert_eq!(param_string(&req, "symbol", ""), "BTC-USDT");
        assert_eq!(param_string(&req, "numeric", "fallback"), "fallback");
        assert_eq!(param_string(&req, "flag", "fallback"), "fallback");
        assert_eq!(body_string(&req, "body_symbol", ""), "ETH-USDT");
        assert_eq!(body_string(&req, "body_numeric", "fallback"), "fallback");
        assert_eq!(body_string(&req, "body_flag", "fallback"), "fallback");
        assert_eq!(
            value_string_at(&json!({"id": 7}), "id", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn string_array_helper_requires_array_of_strings() {
        let req = request(
            json!({
                "comma_list": "BTC,ETH",
                "mixed_list": ["BTC", 1, true, " ETH ", ""]
            }),
            json!({}),
        );

        assert!(request_string_array(&req, "comma_list").is_empty());
        assert_eq!(
            request_string_array(&req, "mixed_list"),
            vec!["BTC".to_string(), "ETH".to_string()]
        );
    }

    #[test]
    fn runtime_helpers_use_top_level_then_params_and_parse_numeric_strings() {
        let req = request(
            json!({}),
            json!({
                "symbol": "SOL-USDT-SWAP",
                "position_size": "0.35",
                "params": {
                    "symbol": "ETH-USDT-SWAP",
                    "timeframe": "15m",
                    "position_size": 0.2,
                    "max_order_value": "250"
                }
            }),
        );
        let params = req
            .body
            .get("params")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        assert_eq!(
            request_runtime_string(&req, &params, "symbol", "BTC-USDT-SWAP"),
            "SOL-USDT-SWAP"
        );
        assert_eq!(
            request_runtime_string(&req, &params, "timeframe", "1H"),
            "15m"
        );
        assert_eq!(
            request_runtime_f64(&req, &params, "position_size", 0.1),
            0.35
        );
        assert_eq!(
            request_runtime_f64_from_params(&params, "max_order_value", 100.0),
            250.0
        );
        assert_eq!(
            request_runtime_f64(&req, &params, "missing_number", 7.0),
            7.0
        );
    }

    #[test]
    fn body_array_helper_requires_native_json_array() {
        let req = request(json!({}), json!({"items": [1, "two"], "text": "1,two"}));

        assert_eq!(
            body_array(&req, "items"),
            vec![Value::from(1), Value::from("two")]
        );
        assert!(body_array(&req, "text").is_empty());
    }
}
