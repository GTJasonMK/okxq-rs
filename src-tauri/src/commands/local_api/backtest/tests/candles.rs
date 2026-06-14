use super::*;

#[tokio::test]
async fn load_backtest_candles_from_db_uses_inclusive_window() {
    let db_path = test_db_path("candles-window");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let base_ts = chrono::Utc::now().timestamp_millis() - 30 * HOUR_MS;
    insert_candles(&pool, &config, base_ts, 30).await;
    let window = BacktestWindow {
        start_ts: base_ts + 2 * HOUR_MS,
        end_ts: base_ts + 25 * HOUR_MS,
        days: 1,
    };

    let candles = load_backtest_candles_from_db(&pool, &config, window)
        .await
        .unwrap();

    assert_eq!(candles.len(), 24);
    assert_eq!(
        candles.first().map(|item| item.timestamp),
        Some(window.start_ts)
    );
    assert_eq!(
        candles.last().map(|item| item.timestamp),
        Some(window.end_ts)
    );

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn load_backtest_context_candles_from_db_includes_pre_window_warmup() {
    let db_path = test_db_path("candles-context-warmup");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let base_ts = chrono::Utc::now().timestamp_millis() - 140 * HOUR_MS;
    insert_candles(&pool, &config, base_ts, 120).await;
    let window = BacktestWindow {
        start_ts: base_ts + 100 * HOUR_MS,
        end_ts: base_ts + 109 * HOUR_MS,
        days: 1,
    };

    let candles = load_backtest_context_candles_from_db(&pool, &config, window, 50)
        .await
        .unwrap();

    assert!(candles.len() >= 50);
    assert!(candles
        .first()
        .is_some_and(|item| item.timestamp < window.start_ts));
    assert!(candles
        .last()
        .is_some_and(|item| item.timestamp <= window.end_ts));
    let warmup_count = candles
        .iter()
        .filter(|item| item.timestamp <= window.start_ts)
        .count();
    assert!(warmup_count >= 50);

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn load_backtest_context_candles_filters_invalid_rows_before_limit() {
    let db_path = test_db_path("candles-context-invalid-limit");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let base_ts = chrono::Utc::now().timestamp_millis() - 40 * HOUR_MS;
    insert_candles(&pool, &config, base_ts, 22).await;
    insert_invalid_close_candle(&pool, &config, base_ts + 22 * HOUR_MS).await;
    let window = BacktestWindow {
        start_ts: base_ts + 21 * HOUR_MS,
        end_ts: base_ts + 22 * HOUR_MS,
        days: 1,
    };

    let candles = load_backtest_context_candles_from_db(&pool, &config, window, 2)
        .await
        .unwrap();

    assert_eq!(candles.len(), 22);
    assert_eq!(candles.first().map(|item| item.timestamp), Some(base_ts));
    assert_eq!(
        candles.last().map(|item| item.timestamp),
        Some(base_ts + 21 * HOUR_MS)
    );

    cleanup_db(pool, &db_path).await;
}
