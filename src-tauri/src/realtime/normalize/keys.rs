use crate::error::{AppError, AppResult};

use super::super::DEFAULT_ORDERBOOK_CHANNEL;

pub(in crate::realtime) fn normalize_inst_id(value: &str) -> AppResult<String> {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    if !normalized.contains('-') {
        return Err(AppError::Validation(
            "inst_id 必须是 OKX 交易对格式，例如 BTC-USDT".to_string(),
        ));
    }
    Ok(normalized)
}

pub(in crate::realtime) fn normalize_orderbook_channel(value: &str) -> AppResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(DEFAULT_ORDERBOOK_CHANNEL.to_string());
    }
    if normalized != DEFAULT_ORDERBOOK_CHANNEL {
        return Err(AppError::Validation(
            "盘口实时通道当前仅支持 OKX books5".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

pub(in crate::realtime) fn normalize_timeframe(value: &str) -> AppResult<String> {
    let mut normalized = value.trim().to_string();
    if normalized.starts_with("candle") {
        normalized = normalized[6..].to_string();
    }
    if normalized.is_empty() {
        return Err(AppError::Validation("timeframe 不能为空".to_string()));
    }
    if !normalized.chars().all(|item| item.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "timeframe 必须是 OKX K 线周期，例如 1m、5m、1H、1D".to_string(),
        ));
    }
    Ok(normalized)
}

pub(in crate::realtime) fn candle_key(inst_id: &str, timeframe: &str) -> String {
    format!("{inst_id}|{timeframe}")
}

pub(in crate::realtime) fn orderbook_key(inst_id: &str, channel: &str) -> String {
    format!("{inst_id}|{channel}")
}

pub(in crate::realtime) fn parse_candle_key(key: &str) -> Option<(String, String)> {
    let (inst_id, timeframe) = key.split_once('|')?;
    if inst_id.is_empty() || timeframe.is_empty() {
        return None;
    }
    Some((inst_id.to_string(), timeframe.to_string()))
}

pub(in crate::realtime) fn parse_orderbook_key(key: &str) -> Option<(String, String)> {
    let (inst_id, channel) = key.split_once('|')?;
    if inst_id.is_empty() || channel.is_empty() {
        return None;
    }
    Some((inst_id.to_string(), channel.to_string()))
}

pub(in crate::realtime) fn candle_channel(timeframe: &str) -> String {
    format!("candle{timeframe}")
}

pub(in crate::realtime) fn timeframe_from_channel(channel: &str) -> AppResult<String> {
    normalize_timeframe(channel)
}
