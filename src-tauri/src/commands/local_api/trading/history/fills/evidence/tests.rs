use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sqlx::Row;

use super::*;

fn temp_db_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
        .join("market.db")
}

#[tokio::test]
async fn upsert_local_fill_backfills_strategy_run_and_arrival_evidence() {
    let db_path = temp_db_path("local_fill_arrival_backfill");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, arrival_ts,
          arrival_mid_px, arrival_bid_px, arrival_ask_px
        ) VALUES (
          'multi_coin_reversion_long_v1', 'Multi Coin Reversion Long',
          'ETH-USDT-SWAP', 'ETH-USDT-SWAP', 'buy', 'market',
          1.5, 3500.0, 'ord-123', 'cl-123', 'live_submitted', 'long',
          '', 'simulated', 1, 'run-abc', 1780000000000, 1780000000456,
          3501.0, 3500.5, 3501.5
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("live order should insert");

    let arrival = lookup_arrival_evidence(&pool, "simulated", "ord-123", "cl-123")
        .await
        .expect("arrival lookup should succeed");
    assert_eq!(arrival.strategy_id, "multi_coin_reversion_long_v1");
    assert_eq!(arrival.run_id, "run-abc");
    assert_eq!(arrival.arrival_ts, Some(1780000000456));
    assert_eq!(arrival.arrival_mid_px, Some(3501.0));
    assert_eq!(arrival.arrival_bid_px, Some(3500.5));
    assert_eq!(arrival.arrival_ask_px, Some(3501.5));

    let fill = json!({
        "fillPx": "3502.25",
        "fillSz": "0.4",
        "fee": "-0.0123",
        "feeCcy": "USDT",
        "side": "buy",
        "ts": "1780000001234"
    });
    upsert_local_fill(UpsertLocalFillRequest {
        db: &pool,
        mode: "simulated",
        trade_id: "trade-123",
        inst_id: "ETH-USDT-SWAP",
        item: &fill,
        order_id: "ord-123",
        client_order_id: "cl-123",
        arrival: &arrival,
    })
    .await
    .expect("local fill should upsert");

    let row = sqlx::query(
        r#"
        SELECT trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
               strategy_id, run_id, arrival_ts, arrival_mid_px,
               arrival_bid_px, arrival_ask_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("trade-123")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("local fill row should exist");

    assert_eq!(row.get::<String, _>("trade_id"), "trade-123");
    assert_eq!(row.get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(row.get::<String, _>("ccy"), "USDT");
    assert_eq!(row.get::<String, _>("side"), "buy");
    assert_eq!(row.get::<String, _>("fill_px"), "3502.25");
    assert_eq!(row.get::<String, _>("fill_sz"), "0.4");
    assert_eq!(row.get::<String, _>("fee"), "-0.0123");
    assert_eq!(row.get::<String, _>("fee_ccy"), "USDT");
    assert_eq!(
        row.get::<String, _>("strategy_id"),
        "multi_coin_reversion_long_v1"
    );
    assert_eq!(row.get::<String, _>("run_id"), "run-abc");
    assert_eq!(row.get::<Option<i64>, _>("arrival_ts"), Some(1780000000456));
    assert_eq!(row.get::<Option<f64>, _>("arrival_mid_px"), Some(3501.0));
    assert_eq!(row.get::<Option<f64>, _>("arrival_bid_px"), Some(3500.5));
    assert_eq!(row.get::<Option<f64>, _>("arrival_ask_px"), Some(3501.5));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn submit_unknown_order_backfills_strategy_run_and_arrival_evidence() {
    let db_path = temp_db_path("local_fill_submit_unknown_arrival");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, arrival_ts,
          arrival_mid_px, arrival_bid_px, arrival_ask_px
        ) VALUES (
          'ml_trade_selector_forward_candidate_v1', 'ML Trade Selector',
          'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'buy', 'market',
          1.0, 70000.0, '', 'cl-submit-unknown', 'submit_unknown', 'open_position',
          'OKX 下单请求已发出，但响应结果待同步确认', 'simulated', 0, 'run-submit-unknown',
          1780000000000, 1780000000123, 70001.0, 70000.5, 70001.5
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("submit unknown order should insert");

    let arrival = lookup_arrival_evidence(
        &pool,
        "simulated",
        "ord-from-okx-after-timeout",
        "cl-submit-unknown",
    )
    .await
    .expect("arrival lookup should succeed");
    assert_eq!(
        arrival.strategy_id,
        "ml_trade_selector_forward_candidate_v1"
    );
    assert_eq!(arrival.run_id, "run-submit-unknown");
    assert_eq!(arrival.arrival_ts, Some(1780000000123));
    assert_eq!(arrival.arrival_mid_px, Some(70001.0));

    let fill = json!({
        "fillPx": "70002",
        "fillSz": "1",
        "fee": "-0.03",
        "feeCcy": "USDT",
        "side": "buy",
        "ts": "1780000000456"
    });
    upsert_local_fill(UpsertLocalFillRequest {
        db: &pool,
        mode: "simulated",
        trade_id: "trade-submit-unknown",
        inst_id: "BTC-USDT-SWAP",
        item: &fill,
        order_id: "ord-from-okx-after-timeout",
        client_order_id: "cl-submit-unknown",
        arrival: &arrival,
    })
    .await
    .expect("local fill should upsert");

    let row = sqlx::query(
        r#"
        SELECT strategy_id, run_id, arrival_mid_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("trade-submit-unknown")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("local fill row should exist");
    assert_eq!(
        row.get::<String, _>("strategy_id"),
        "ml_trade_selector_forward_candidate_v1"
    );
    assert_eq!(row.get::<String, _>("run_id"), "run-submit-unknown");
    assert_eq!(row.get::<Option<f64>, _>("arrival_mid_px"), Some(70001.0));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn upsert_local_fill_keeps_missing_fee_unknown() {
    let db_path = temp_db_path("local_fill_missing_fee");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    let fill = json!({
        "fillPx": "70000",
        "fillSz": "0.01",
        "side": "buy",
        "ts": "1780000001234"
    });
    upsert_local_fill(UpsertLocalFillRequest {
        db: &pool,
        mode: "simulated",
        trade_id: "trade-missing-fee",
        inst_id: "BTC-USDT-SWAP",
        item: &fill,
        order_id: "ord-missing-fee",
        client_order_id: "cl-missing-fee",
        arrival: &ArrivalEvidence::default(),
    })
    .await
    .expect("local fill should upsert");

    let row = sqlx::query(
        r#"
        SELECT fee
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("trade-missing-fee")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("local fill row should exist");

    assert_eq!(row.get::<String, _>("fee"), "");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn incomplete_arrival_quote_keeps_strategy_but_not_slippage_evidence() {
    let db_path = temp_db_path("local_fill_incomplete_arrival");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp,
          arrival_mid_px, arrival_bid_px, arrival_ask_px
        ) VALUES (
          'multi_coin_reversion_long_v1', 'Multi Coin Reversion Long',
          'SOL-USDT-SWAP', 'SOL-USDT-SWAP', 'buy', 'market',
          3.0, 160.0, 'ord-incomplete', 'cl-incomplete',
          'live_submitted', 'long', '', 'simulated', 1, 'run-incomplete',
          1780000000000, 160.2, NULL, NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("live order should insert");

    let arrival = lookup_arrival_evidence(&pool, "simulated", "ord-incomplete", "cl-incomplete")
        .await
        .expect("arrival lookup should succeed");
    assert_eq!(arrival.strategy_id, "multi_coin_reversion_long_v1");
    assert_eq!(arrival.run_id, "run-incomplete");
    assert_eq!(arrival.arrival_ts, None);
    assert_eq!(arrival.arrival_mid_px, None);
    assert_eq!(arrival.arrival_bid_px, None);
    assert_eq!(arrival.arrival_ask_px, None);

    let fill = json!({
        "fillPx": "160.25",
        "fillSz": "1.0",
        "fee": "-0.001",
        "feeCcy": "USDT",
        "side": "buy",
        "ts": "1780000001234"
    });
    upsert_local_fill(UpsertLocalFillRequest {
        db: &pool,
        mode: "simulated",
        trade_id: "trade-incomplete",
        inst_id: "SOL-USDT-SWAP",
        item: &fill,
        order_id: "ord-incomplete",
        client_order_id: "cl-incomplete",
        arrival: &arrival,
    })
    .await
    .expect("local fill should upsert");

    let row = sqlx::query(
        r#"
        SELECT strategy_id, run_id, arrival_ts, arrival_mid_px,
               arrival_bid_px, arrival_ask_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("trade-incomplete")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("local fill row should exist");

    assert_eq!(
        row.get::<String, _>("strategy_id"),
        "multi_coin_reversion_long_v1"
    );
    assert_eq!(row.get::<String, _>("run_id"), "run-incomplete");
    assert_eq!(row.get::<Option<i64>, _>("arrival_ts"), None);
    assert_eq!(row.get::<Option<f64>, _>("arrival_mid_px"), None);
    assert_eq!(row.get::<Option<f64>, _>("arrival_bid_px"), None);
    assert_eq!(row.get::<Option<f64>, _>("arrival_ask_px"), None);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn blocked_order_records_do_not_backfill_fill_evidence() {
    let db_path = temp_db_path("local_fill_ignores_blocked_order_evidence");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
            INSERT INTO live_order_records (
              strategy_id, strategy_name, symbol, inst_id, side, order_type,
              size, price, order_id, client_order_id, status, action,
              error_message, mode, success, run_id, action_timestamp, arrival_ts,
              arrival_mid_px, arrival_bid_px, arrival_ask_px
            ) VALUES (
              'spread_velocity_v1', 'Spread Velocity',
              'SOL-USDT-SWAP', 'SOL-USDT-SWAP', 'sell', 'market',
              1.0, 100.0, 'ord-blocked', 'cl-blocked', 'blocked', 'close_position',
              '', 'simulated', 0, 'run-blocked', 1780000000000, 1780000000456,
              100.0, 99.9, 100.1
            )
            "#,
    )
    .execute(&pool)
    .await
    .expect("order should insert");

    let blocked_arrival = lookup_arrival_evidence(&pool, "simulated", "ord-blocked", "cl-blocked")
        .await
        .expect("blocked arrival lookup should not fail");
    assert_eq!(blocked_arrival.strategy_id, "");
    assert_eq!(blocked_arrival.run_id, "");
    assert_eq!(blocked_arrival.arrival_ts, None);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn arrival_evidence_rejects_conflicting_order_and_client_id_matches() {
    let db_path = temp_db_path("arrival_evidence_latest_identity_match");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    insert_live_order_for_arrival_test(
        &pool,
        "older-strategy",
        "older-run",
        "ord-shared",
        "cl-older",
        "2026-06-06T00:00:01Z",
    )
    .await;
    insert_live_order_for_arrival_test(
        &pool,
        "newer-strategy",
        "newer-run",
        "ord-newer",
        "cl-shared",
        "2026-06-06T00:00:02Z",
    )
    .await;

    let error = lookup_arrival_evidence(&pool, "simulated", "ord-shared", "cl-shared")
        .await
        .expect_err("conflicting order/client identity matches must be rejected");

    assert!(error.to_string().contains("不同订单记录"));

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn arrival_evidence_rejects_duplicate_client_identity_matches() {
    let db_path = temp_db_path("arrival_evidence_duplicate_client_identity");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    insert_live_order_for_arrival_test(
        &pool,
        "older-strategy",
        "older-run",
        "ord-older",
        "cl-duplicate",
        "2026-06-06T00:00:01Z",
    )
    .await;
    insert_live_order_for_arrival_test(
        &pool,
        "newer-strategy",
        "newer-run",
        "ord-newer",
        "cl-duplicate",
        "2026-06-06T00:00:02Z",
    )
    .await;

    let error = lookup_arrival_evidence(&pool, "simulated", "", "cl-duplicate")
        .await
        .expect_err("duplicate client identity matches must be rejected");

    assert!(error.to_string().contains("多条订单记录"));

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn arrival_evidence_lookup_uses_identity_recent_indexes() {
    let db_path = temp_db_path("arrival_evidence_identity_indexes");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    let order_plan = arrival_identity_query_plan(&pool, "order_id")
        .await
        .join("\n");
    let client_plan = arrival_identity_query_plan(&pool, "client_order_id")
        .await
        .join("\n");

    assert!(
        order_plan.contains("idx_live_orders_mode_order_recent"),
        "order_id arrival lookup should use mode/order/recent index, got:\n{order_plan}"
    );
    assert!(
        client_plan.contains("idx_live_orders_mode_client_recent"),
        "client_order_id arrival lookup should use mode/client/recent index, got:\n{client_plan}"
    );
    assert!(
        !order_plan.contains("USE TEMP B-TREE"),
        "order_id arrival lookup should not sort with temp b-tree, got:\n{order_plan}"
    );
    assert!(
        !client_plan.contains("USE TEMP B-TREE"),
        "client_order_id arrival lookup should not sort with temp b-tree, got:\n{client_plan}"
    );

    cleanup_db(pool, &db_path).await;
}

async fn insert_live_order_for_arrival_test(
    pool: &sqlx::SqlitePool,
    strategy_id: &str,
    run_id: &str,
    order_id: &str,
    client_order_id: &str,
    created_at: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, arrival_ts,
          arrival_mid_px, arrival_bid_px, arrival_ask_px, created_at
        ) VALUES (
          ?, 'Arrival Test', 'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'buy', 'market',
          1.0, 100.0, ?, ?, 'live_submitted', 'long',
          '', 'simulated', 1, ?, 1780000000000, 1780000000456,
          100.0, 99.9, 100.1, ?
        )
        "#,
    )
    .bind(strategy_id)
    .bind(order_id)
    .bind(client_order_id)
    .bind(run_id)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("insert live order arrival test row");
}

async fn arrival_identity_query_plan(
    pool: &sqlx::SqlitePool,
    identity_column: &str,
) -> Vec<String> {
    let sql = format!(
        r#"
        EXPLAIN QUERY PLAN
        SELECT id
        FROM live_order_records
        WHERE mode = ?
          AND {identity_column} = ?
          AND (
            COALESCE(success, 0) = 1
            OR LOWER(TRIM(COALESCE(status, ''))) IN (
              'submitting', 'submit_unknown', 'submitted', 'pending', 'open', 'live',
              'partially_filled', 'partial-filled', 'partially-filled',
              'cancel_requested', 'modify_requested',
              'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
              'algo_cancel_requested', 'algo_modify_requested',
              'algo_partially_effective', 'algo_effective'
            )
          )
          AND LOWER(TRIM(COALESCE(status, ''))) NOT IN (
            'blocked', 'risk_blocked', 'submit_failed', 'algo_failed', 'rejected', 'reject'
          )
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#
    );
    sqlx::query(&sql)
        .bind("simulated")
        .bind("identity")
        .fetch_all(pool)
        .await
        .expect("arrival identity explain")
        .into_iter()
        .map(|row| row.try_get::<String, _>("detail").unwrap_or_default())
        .collect()
}

async fn cleanup_db(pool: sqlx::SqlitePool, db_path: &std::path::Path) {
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
