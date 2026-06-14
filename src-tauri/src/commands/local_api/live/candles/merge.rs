use crate::{
    commands::local_api::LocalApiRequest,
    error::{AppError, AppResult},
    okx::OkxCandle,
};

use super::{
    canonical_diagnostic_timeframe,
    json::{json_f64, json_i64, json_string},
};

pub(in crate::commands::local_api::live) fn merge_latest_diagnostic_candle(
    req: &LocalApiRequest,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    limit: usize,
    candles: &mut Vec<OkxCandle>,
) -> AppResult<bool> {
    let Some(raw) = req.body.get("latest_candle") else {
        return Ok(false);
    };
    if raw.is_null() {
        return Ok(false);
    }
    let Some(item) = raw.as_object() else {
        return Err(AppError::Validation(
            "latest_candle 必须是 K 线对象".to_string(),
        ));
    };

    let candle_symbol = required_candle_string(item, "inst_id")?
        .trim()
        .to_uppercase();
    let candle_inst_type = required_candle_string(item, "inst_type")?
        .trim()
        .to_uppercase();
    let candle_timeframe =
        canonical_diagnostic_timeframe(&required_candle_string(item, "timeframe")?);
    let expected_timeframe = canonical_diagnostic_timeframe(timeframe);

    if candle_symbol != symbol
        || candle_inst_type != inst_type
        || candle_timeframe != expected_timeframe
    {
        return Ok(false);
    }

    let latest = OkxCandle {
        timestamp: json_i64(item.get("timestamp")).ok_or_else(|| {
            AppError::Validation("latest_candle.timestamp 必须是有效毫秒时间戳".to_string())
        })?,
        open: json_f64(item.get("open"))
            .ok_or_else(|| AppError::Validation("latest_candle.open 必须是有效数字".to_string()))?,
        high: json_f64(item.get("high"))
            .ok_or_else(|| AppError::Validation("latest_candle.high 必须是有效数字".to_string()))?,
        low: json_f64(item.get("low"))
            .ok_or_else(|| AppError::Validation("latest_candle.low 必须是有效数字".to_string()))?,
        close: json_f64(item.get("close")).ok_or_else(|| {
            AppError::Validation("latest_candle.close 必须是有效数字".to_string())
        })?,
        volume: json_f64(item.get("volume")).ok_or_else(|| {
            AppError::Validation("latest_candle.volume 必须是有效数字".to_string())
        })?,
        volume_ccy: non_negative_candle_f64(item, "volume_ccy")?,
        volume_quote: non_negative_candle_f64(item, "volume_quote")?,
        confirm: candle_confirm(item)?,
    };
    if latest.timestamp <= 0 {
        return Err(AppError::Validation(
            "latest_candle.timestamp 必须大于 0".to_string(),
        ));
    }
    if !latest.is_valid_market_candle() {
        return Err(AppError::Validation(
            "latest_candle 必须包含正数 OHLC 和非负成交量".to_string(),
        ));
    }

    if let Some(index) = candles
        .iter()
        .position(|candle| candle.timestamp == latest.timestamp)
    {
        candles[index] = latest;
    } else {
        candles.push(latest);
    }
    candles.sort_by_key(|candle| candle.timestamp);
    if candles.len() > limit {
        candles.drain(0..candles.len() - limit);
    }
    Ok(true)
}

fn required_candle_string(
    item: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> AppResult<String> {
    json_string(item.get(key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("latest_candle.{key} 必须是非空字符串")))
}

fn non_negative_candle_f64(
    item: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> AppResult<f64> {
    let value = json_f64(item.get(key))
        .filter(|value| *value >= 0.0)
        .ok_or_else(|| AppError::Validation(format!("latest_candle.{key} 必须是非负数字")))?;
    Ok(value)
}

fn candle_confirm(item: &serde_json::Map<String, serde_json::Value>) -> AppResult<String> {
    match item.get("confirm").and_then(|value| value.as_str()) {
        Some("0") => Ok("0".to_string()),
        Some("1") => Ok("1".to_string()),
        _ => Err(AppError::Validation(
            "latest_candle.confirm 必须是 0 或 1".to_string(),
        )),
    }
}
