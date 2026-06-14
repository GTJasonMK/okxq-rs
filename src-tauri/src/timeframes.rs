pub(crate) fn normalize_okx_timeframe(value: &str) -> Option<&'static str> {
    match value.trim() {
        "1m" => Some("1m"),
        "3m" => Some("3m"),
        "5m" => Some("5m"),
        "15m" => Some("15m"),
        "30m" => Some("30m"),
        "1H" | "1h" => Some("1H"),
        "2H" | "2h" => Some("2H"),
        "4H" | "4h" => Some("4H"),
        "6H" | "6h" => Some("6H"),
        "12H" | "12h" => Some("12H"),
        "1D" | "1d" => Some("1D"),
        "1W" | "1w" => Some("1W"),
        "1M" => Some("1M"),
        _ => None,
    }
}

pub(crate) fn sort_okx_timeframes(values: &mut Vec<String>) {
    values.sort_by_key(|timeframe| okx_timeframe_order(timeframe));
    values.dedup();
}

pub(crate) fn normalized_okx_timeframe_to_ms(timeframe: &str) -> Option<i64> {
    const MINUTE_MS: i64 = 60_000;
    const DAY_MS: i64 = 86_400_000;
    match timeframe.trim() {
        "1m" => Some(MINUTE_MS),
        "3m" => Some(3 * MINUTE_MS),
        "5m" => Some(5 * MINUTE_MS),
        "15m" => Some(15 * MINUTE_MS),
        "30m" => Some(30 * MINUTE_MS),
        "1H" => Some(60 * MINUTE_MS),
        "2H" => Some(2 * 60 * MINUTE_MS),
        "4H" => Some(4 * 60 * MINUTE_MS),
        "6H" => Some(6 * 60 * MINUTE_MS),
        "12H" => Some(12 * 60 * MINUTE_MS),
        "1D" => Some(DAY_MS),
        "1W" => Some(7 * DAY_MS),
        "1M" => Some(30 * DAY_MS),
        _ => None,
    }
}

pub(crate) fn okx_timeframe_millis_or(timeframe: &str, default_ms: i64) -> i64 {
    normalize_okx_timeframe(timeframe)
        .and_then(normalized_okx_timeframe_to_ms)
        .unwrap_or(default_ms)
}

pub(crate) fn okx_timeframe_order(value: &str) -> i64 {
    match value {
        "1m" => 1,
        "3m" => 2,
        "5m" => 3,
        "15m" => 4,
        "30m" => 5,
        "1H" => 6,
        "2H" => 7,
        "4H" => 8,
        "6H" => 9,
        "12H" => 10,
        "1D" => 11,
        "1W" => 12,
        "1M" => 13,
        _ => 999,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn okx_timeframe_rules_normalize_sort_and_measure_consistently() {
        let mut values = vec![
            "1D".to_string(),
            "1m".to_string(),
            "1H".to_string(),
            "1H".to_string(),
            "3m".to_string(),
        ];
        sort_okx_timeframes(&mut values);

        assert_eq!(normalize_okx_timeframe("4h"), Some("4H"));
        assert_eq!(normalize_okx_timeframe("bad"), None);
        assert_eq!(values, vec!["1m", "3m", "1H", "1D"]);
        assert_eq!(normalized_okx_timeframe_to_ms("1H"), Some(3_600_000));
        assert_eq!(normalized_okx_timeframe_to_ms("1h"), None);
        assert_eq!(okx_timeframe_millis_or("4h", 900_000), 14_400_000);
        assert_eq!(okx_timeframe_millis_or("bad", 900_000), 900_000);
    }
}
