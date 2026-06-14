use serde_json::Value;

use crate::okx::OkxCandle;

pub(in crate::commands::local_api::live) fn candle_to_json(candle: &OkxCandle) -> Value {
    candle.to_json_with_confirm()
}

pub(super) fn json_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

pub(super) fn json_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
    })
}

pub(super) fn json_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = value.and_then(Value::as_f64)?;
    parsed.is_finite().then_some(parsed)
}
