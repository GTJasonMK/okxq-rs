use serde_json::Value;

pub(in crate::commands::local_api::backtest::runner) fn no_evaluable_window_message(
    detail: &Value,
) -> String {
    let first_evaluable = detail
        .get("first_evaluable_time")
        .and_then(Value::as_str)
        .expect("warmup detail first_evaluable_time should be a string");
    let last_window = detail
        .get("last_window_time")
        .and_then(Value::as_str)
        .expect("warmup detail last_window_time should be a string");
    let blockers = detail
        .get("blocked_requirements")
        .and_then(Value::as_array)
        .expect("warmup detail blocked_requirements should be an array")
        .iter()
        .take(4)
        .map(|item| {
            let symbol = item
                .get("symbol")
                .and_then(Value::as_str)
                .expect("warmup blocker symbol should be a string");
            let timeframe = item
                .get("timeframe")
                .and_then(Value::as_str)
                .expect("warmup blocker timeframe should be a string");
            let available = item
                .get("available_until_window_end")
                .and_then(Value::as_u64)
                .expect("warmup blocker available_until_window_end should be an integer");
            let required = item
                .get("min_bars")
                .and_then(Value::as_u64)
                .expect("warmup blocker min_bars should be an integer");
            format!("{symbol} {timeframe} 可用 {available}/{required}")
        })
        .collect::<Vec<_>>();
    assert!(
        !blockers.is_empty(),
        "warmup detail should include blocked requirements"
    );
    let blocker_text = blockers.join("；");
    format!(
        "回测窗口内没有任何满足 DATA_REQUIREMENTS warmup 的历史决策点。最早可评估时间 {first_evaluable} 晚于窗口最后K线 {last_window}；{blocker_text}。请扩大回测窗口、提前同步 warmup 所需历史数据，或降低策略 DATA_REQUIREMENTS.min_bars。"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::no_evaluable_window_message;

    #[test]
    fn no_evaluable_window_message_reads_canonical_warmup_detail() {
        let message = no_evaluable_window_message(&json!({
            "first_evaluable_time": "2026-01-01T00:10:00+00:00",
            "last_window_time": "2026-01-01T00:05:00+00:00",
            "blocked_requirements": [
                {
                    "symbol": "BTC-USDT-SWAP",
                    "timeframe": "1m",
                    "available_until_window_end": 2,
                    "min_bars": 5
                }
            ]
        }));

        assert!(message.contains("BTC-USDT-SWAP 1m 可用 2/5"));
        assert!(message.contains("2026-01-01T00:10:00+00:00"));
        assert!(message.contains("2026-01-01T00:05:00+00:00"));
    }

    #[test]
    #[should_panic(expected = "warmup detail blocked_requirements should be an array")]
    fn no_evaluable_window_message_requires_canonical_blocker_array() {
        no_evaluable_window_message(&json!({
            "first_evaluable_time": "2026-01-01T00:10:00+00:00",
            "last_window_time": "2026-01-01T00:05:00+00:00"
        }));
    }
}
