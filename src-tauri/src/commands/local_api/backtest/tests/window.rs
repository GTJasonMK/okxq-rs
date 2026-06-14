use super::*;

#[test]
fn backtest_window_uses_explicit_inclusive_date_range() {
    let req = request(json!({
        "start_date": "2026-05-01",
        "end_date": "2026-05-10",
        "days": 365
    }));

    let window = backtest_window(&req, 365).unwrap();

    assert_eq!(window.start_ts, utc_ms(2026, 5, 1, 0, 0, 0, 0));
    assert_eq!(window.end_ts, utc_ms(2026, 5, 10, 23, 59, 59, 999));
    assert_eq!(window.days, 10);
}

#[test]
fn backtest_window_uses_days_fallback_when_start_is_missing() {
    let req = request(json!({
        "end_date": "2026-05-10",
        "days": 3
    }));

    let window = backtest_window(&req, 365).unwrap();

    assert_eq!(window.end_ts, utc_ms(2026, 5, 10, 23, 59, 59, 999));
    assert_eq!(window.start_ts, window.end_ts - 3 * DAY_MS);
    assert_eq!(window.days, 3);
}

#[test]
fn backtest_window_rejects_reversed_dates() {
    let req = request(json!({
        "start_date": "2026-05-10",
        "end_date": "2026-05-01"
    }));

    let error = backtest_window(&req, 365).unwrap_err().to_string();

    assert!(error.contains("回测结束日期必须晚于开始日期"));
}

#[test]
fn backtest_window_rejects_invalid_date_text() {
    let req = request(json!({
        "start_date": "not-a-date",
        "end_date": "2026-05-10"
    }));

    let error = backtest_window(&req, 365).unwrap_err().to_string();

    assert!(error.contains("回测开始日期无效"));
}

#[test]
fn backtest_persist_flag_defaults_to_persisted_results() {
    assert!(should_persist_backtest_result(&request(json!({}))));

    assert!(!should_persist_backtest_result(&request(json!({
        "persist_result": false
    }))));
}

#[test]
fn expected_backtest_candle_count_uses_timeframe_not_days() {
    let window = BacktestWindow {
        start_ts: 0,
        end_ts: DAY_MS - 1,
        days: 1,
    };

    assert_eq!(expected_backtest_candle_count("1H", window), 24);
    assert_eq!(expected_backtest_candle_count("15m", window), 96);
    assert_eq!(
        expected_backtest_candle_count(
            "1m",
            BacktestWindow {
                start_ts: 0,
                end_ts: 3650 * DAY_MS,
                days: 3650,
            },
        ),
        MAX_BACKTEST_CANDLES
    );
}
