use super::*;

use crate::timeframes::okx_timeframe_millis_or;

pub(super) const DAY_MS: i64 = 86_400_000;
pub(super) const MAX_BACKTEST_CANDLES: i64 = 300_000;

#[derive(Clone, Copy, Debug)]
pub(super) struct BacktestWindow {
    pub(super) start_ts: i64,
    pub(super) end_ts: i64,
    pub(super) days: i64,
}

pub(super) fn backtest_window(
    req: &LocalApiRequest,
    default_days: i64,
) -> AppResult<BacktestWindow> {
    let fallback_days = request_i64(req, "days", default_days).clamp(1, 3650);
    let now = chrono::Utc::now().timestamp_millis();
    let start_text = request_string(req, "start_date", "");
    let end_text = request_string(req, "end_date", "");

    if start_text.trim().is_empty() && end_text.trim().is_empty() {
        return Ok(BacktestWindow {
            start_ts: now.saturating_sub(fallback_days.saturating_mul(DAY_MS)),
            end_ts: now,
            days: fallback_days,
        });
    }

    let end_ts = if end_text.trim().is_empty() {
        now
    } else {
        parse_backtest_date_millis(&end_text, true)
            .ok_or_else(|| AppError::Validation(format!("回测结束日期无效: {end_text}")))?
    };
    let start_ts = if start_text.trim().is_empty() {
        end_ts.saturating_sub(fallback_days.saturating_mul(DAY_MS))
    } else {
        parse_backtest_date_millis(&start_text, false)
            .ok_or_else(|| AppError::Validation(format!("回测开始日期无效: {start_text}")))?
    };
    if end_ts <= start_ts {
        return Err(AppError::Validation(
            "回测结束日期必须晚于开始日期".to_string(),
        ));
    }
    let days = ((end_ts - start_ts + DAY_MS - 1) / DAY_MS).clamp(1, 3650);
    Ok(BacktestWindow {
        start_ts,
        end_ts,
        days,
    })
}

fn parse_backtest_date_millis(raw: &str, end_of_day: bool) -> Option<i64> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(timestamp) = value.parse::<i64>() {
        return Some(timestamp);
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let start = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
        return Some(if end_of_day {
            start.saturating_add(DAY_MS - 1)
        } else {
            start
        });
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|item| item.timestamp_millis())
}

pub(super) fn expected_backtest_candle_count(timeframe: &str, window: BacktestWindow) -> i64 {
    let timeframe_ms = okx_timeframe_millis_or(timeframe, DAY_MS).max(1);
    let span_ms = window.end_ts.saturating_sub(window.start_ts).max(0);
    span_ms
        .saturating_div(timeframe_ms)
        .saturating_add(1)
        .clamp(20, MAX_BACKTEST_CANDLES)
}

pub(super) fn should_persist_backtest_result(req: &LocalApiRequest) -> bool {
    body_bool(req, "persist_result", true)
}
