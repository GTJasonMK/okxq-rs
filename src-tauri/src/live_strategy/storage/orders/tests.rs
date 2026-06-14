use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sqlx::Row;

use super::super::super::{arrival::ArrivalQuote, types::LiveStrategyConfig};
use super::{
    insert::{insert_live_exchange_order, insert_live_order, insert_live_order_with_type},
    query::{query_live_order_context, query_live_orders},
    sync::{query_live_fill_sync_scopes, query_live_order_sync_candidates},
};

fn test_config() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "arrival_storage_test".to_string(),
        strategy_name: "Arrival Storage Test".to_string(),
        symbol: "ETH-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1000.0,
        position_size: 0.35,
        stop_loss: 0.0,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({}),
        project_root: PathBuf::from("."),
        risk_control_enabled: false,
        max_single_loss_ratio: 0.0,
        max_position_pct: 0.0,
        max_order_value: 0.0,
    }
}

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
async fn insert_live_order_leaves_arrival_null_without_real_quote() {
    let db_path = temp_db_path("arrival_null_without_quote");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();

    insert_live_order(
        &pool,
        &config,
        "buy",
        1.0,
        1234.5,
        "open_position",
        "filled",
        true,
        "unit test",
        "run-arrival-null",
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("order should insert");

    let row = sqlx::query(
        r#"
        SELECT price, arrival_mid_px, arrival_bid_px, arrival_ask_px
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-arrival-null")
    .fetch_one(&pool)
    .await
    .expect("order row should exist");

    assert_eq!(row.get::<f64, _>("price"), 1234.5);
    assert_eq!(row.get::<Option<f64>, _>("arrival_mid_px"), None);
    assert_eq!(row.get::<Option<f64>, _>("arrival_bid_px"), None);
    assert_eq!(row.get::<Option<f64>, _>("arrival_ask_px"), None);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn insert_live_order_persists_real_arrival_quote() {
    let db_path = temp_db_path("arrival_real_quote");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let arrival = ArrivalQuote {
        ts_ms: Some(1_780_000_015_123),
        mid_px: Some(100.0),
        bid_px: Some(99.5),
        ask_px: Some(100.5),
    };

    insert_live_order(
        &pool,
        &config,
        "sell",
        2.0,
        101.25,
        "open_position",
        "filled",
        true,
        "unit test",
        "run-arrival-real",
        1_780_000_015_000,
        arrival,
    )
    .await
    .expect("order should insert");

    let row = sqlx::query(
        r#"
        SELECT arrival_ts, arrival_mid_px, arrival_bid_px, arrival_ask_px
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-arrival-real")
    .fetch_one(&pool)
    .await
    .expect("order row should exist");

    assert_eq!(
        row.get::<Option<i64>, _>("arrival_ts"),
        Some(1_780_000_015_123)
    );
    assert_eq!(row.get::<Option<f64>, _>("arrival_mid_px"), Some(100.0));
    assert_eq!(row.get::<Option<f64>, _>("arrival_bid_px"), Some(99.5));
    assert_eq!(row.get::<Option<f64>, _>("arrival_ask_px"), Some(100.5));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn insert_live_exchange_order_rejects_duplicate_active_parent_identity() {
    let db_path = temp_db_path("duplicate_active_parent_identity");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();

    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "submitted",
        "run-duplicate-parent-1",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "duplicate-parent-order",
        "duplicateparentclient",
    )
    .await
    .expect("first parent identity should insert");

    let error = insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "submitted again",
        "run-duplicate-parent-2",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "duplicate-parent-order",
        "duplicateparentclient",
    )
    .await
    .expect_err("duplicate active parent identity must be rejected");

    assert!(error.to_string().contains("普通订单身份"));
    assert!(error.to_string().contains("已拒绝写入或更新"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn insert_live_order_rejects_non_positive_size_or_price_before_persisting() {
    let db_path = temp_db_path("reject_invalid_order_economics");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();

    let zero_size = insert_live_order(
        &pool,
        &config,
        "buy",
        0.0,
        100.0,
        "open_position",
        "filled",
        true,
        "zero size should not persist",
        "run-zero-size",
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await;
    let zero_price = insert_live_order(
        &pool,
        &config,
        "buy",
        1.0,
        0.0,
        "open_position",
        "filled",
        true,
        "zero price should not persist",
        "run-zero-price",
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await;

    assert!(zero_size.is_err());
    assert!(zero_price.is_err());
    let rows =
        sqlx::query("SELECT COUNT(*) AS count FROM live_order_records WHERE run_id IN (?, ?)")
            .bind("run-zero-size")
            .bind("run-zero-price")
            .fetch_one(&pool)
            .await
            .expect("count query should run");
    assert_eq!(rows.get::<i64, _>("count"), 0);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn insert_live_exchange_order_allows_unknown_size_for_order_management_candidate() {
    let db_path = temp_db_path("order_management_unknown_size");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();

    let row_id = insert_live_exchange_order(
        &pool,
        &config,
        "hold",
        "market",
        0.0,
        0.0,
        "cancel_order",
        "cancel_requested",
        true,
        "external cancel request accepted",
        "run-order-management-unknown-size",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "external-order-id",
        "externalclientid",
    )
    .await
    .expect("order management sync candidates should not need a fabricated size");

    let stored = sqlx::query("SELECT size, price FROM live_order_records WHERE id = ?")
        .bind(row_id)
        .fetch_one(&pool)
        .await
        .expect("stored candidate should query");
    assert_eq!(stored.get::<f64, _>("size"), 0.0);
    assert_eq!(stored.get::<Option<f64>, _>("price"), None);

    let orders = query_live_orders(&pool, 10, &config.mode, "run-order-management-unknown-size")
        .await
        .expect("orders should normalize");
    assert_eq!(orders.len(), 1);
    assert!(orders[0]["size"].is_null());
    assert!(orders[0]["value"].is_null());

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn insert_live_order_allows_unknown_price_for_market_close_record() {
    let db_path = temp_db_path("market_close_unknown_price");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();

    let row_id = insert_live_order(
        &pool,
        &config,
        "sell",
        1.0,
        0.0,
        "close_position",
        "submitted",
        true,
        "market close submitted; fill price comes from OKX fills",
        "run-market-close-unknown-price",
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("market close without strategy price should persist with unknown price");

    let row = sqlx::query("SELECT price FROM live_order_records WHERE id = ?")
        .bind(row_id)
        .fetch_one(&pool)
        .await
        .expect("order should persist");
    assert_eq!(row.get::<Option<f64>, _>("price"), None);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_context_groups_open_fills_and_rejections() {
    let db_path = temp_db_path("order_context_groups");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-order-context";

    insert_live_order(
        &pool,
        &config,
        "buy",
        1.0,
        100.0,
        "open_position",
        "filled",
        true,
        "entry",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("filled order should insert");
    insert_live_order_with_type(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "protective stop",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("open risk order should insert");
    insert_live_order(
        &pool,
        &config,
        "buy",
        1.0,
        100.0,
        "open_position",
        "risk_blocked",
        false,
        "blocked",
        run_id,
        1_780_000_060_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("rejected order should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        1.0,
        100.0,
        "open_position",
        "cancel_requested",
        true,
        "cancel requested",
        run_id,
        1_780_000_120_000,
        ArrivalQuote::default(),
        "cancel-requested-order",
        "cancelrequestedclient",
    )
    .await
    .expect("cancel requested order should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "modify_requested",
        true,
        "modify requested",
        run_id,
        1_780_000_180_000,
        ArrivalQuote::default(),
        "modify-requested-order",
        "modifyrequestedclient",
    )
    .await
    .expect("modify requested order should insert");

    let status = crate::live_strategy::LiveStrategyStatus {
        run_id: run_id.to_string(),
        mode: "simulated".to_string(),
        total_orders: 5,
        successful_orders: 4,
        failed_orders: 1,
        ..crate::live_strategy::LiveStrategyStatus::default()
    };
    let context = query_live_order_context(&pool, &status)
        .await
        .expect("order context should query");

    let open = context["open"]
        .as_array()
        .expect("open orders should exist");
    assert_eq!(open.len(), 3);
    assert!(
        open.iter()
            .any(|item| item["action"].as_str() == Some("place_risk_order")),
        "protective algo order should stay open"
    );
    assert!(
        open.iter()
            .any(|item| item["status"].as_str() == Some("cancel_requested")),
        "cancel requested exchange order should stay open for strategy context"
    );
    assert!(
        open.iter()
            .any(|item| item["status"].as_str() == Some("modify_requested")),
        "modify requested exchange order should stay open for strategy context"
    );
    assert_eq!(context["recent_fills"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        context["recent_fills"][0]["action"].as_str(),
        Some("open_position")
    );
    assert_eq!(context["recent_fills"][0]["size"].as_f64(), Some(1.0));
    assert_eq!(context["recent_fills"][0]["quantity"].as_f64(), Some(1.0));
    assert_eq!(context["recent_fills"][0]["value"].as_f64(), Some(100.0));
    assert_eq!(
        context["recent_rejections"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        context["recent_rejections"][0]["action"].as_str(),
        Some("open_position")
    );
    assert_eq!(context["total_orders"].as_i64(), Some(5));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_enriches_exchange_orders_with_actual_fills() {
    let db_path = temp_db_path("live_orders_actual_fill_aggregate");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-fill-aggregate";
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        3.0,
        100.0,
        "open_position",
        "partially_filled",
        true,
        "submitted",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
        "order-fill-aggregate",
        "client-fill-aggregate",
    )
    .await
    .expect("exchange order should insert");
    insert_local_fill_for_test(
        &pool,
        "trade-fill-1",
        "order-fill-aggregate",
        "client-fill-aggregate",
        "100",
        "1",
        "-0.01",
        1_780_000_000_100,
    )
    .await;
    insert_local_fill_for_test(
        &pool,
        "trade-fill-2",
        "order-fill-aggregate",
        "client-fill-aggregate",
        "110",
        "2",
        "-0.02",
        1_780_000_000_200,
    )
    .await;

    let orders = query_live_orders(&pool, 10, "simulated", run_id)
        .await
        .expect("orders should query");

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["fill_count"].as_i64(), Some(2));
    assert_eq!(orders[0]["filled_size"].as_f64(), Some(3.0));
    assert_eq!(orders[0]["filled_quantity"].as_f64(), Some(3.0));
    assert_eq!(orders[0]["fill_notional"].as_f64(), Some(320.0));
    assert_eq!(orders[0]["avg_fill_price"].as_f64(), Some(320.0 / 3.0));
    assert_eq!(orders[0]["remaining_size"].as_f64(), Some(0.0));
    assert_eq!(orders[0]["total_fee"].as_f64(), Some(-0.03));
    assert_eq!(orders[0]["fee_ccy"].as_str(), Some("USDT"));
    assert_eq!(orders[0]["first_fill_ts"].as_i64(), Some(1_780_000_000_100));
    assert_eq!(orders[0]["last_fill_ts"].as_i64(), Some(1_780_000_000_200));
    assert_eq!(orders[0]["fill_source"].as_str(), Some("okx_private_ws"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_enriches_algo_orders_with_actual_order_fills() {
    let db_path = temp_db_path("live_orders_algo_actual_fill_aggregate");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-algo-fill-aggregate";
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_effective",
        true,
        "protective algo triggered",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-fill-aggregate",
        "clalgoaggregate",
    )
    .await
    .expect("algo order should insert");
    sqlx::query(
        r#"
        UPDATE live_order_records
        SET actual_order_id = ?, actual_client_order_id = ?
        WHERE order_id = ?
        "#,
    )
    .bind("actual-fill-aggregate")
    .bind("actual-client-aggregate")
    .bind("algo-fill-aggregate")
    .execute(&pool)
    .await
    .expect("actual identity should update");
    insert_local_fill_for_test(
        &pool,
        "trade-algo-fill-1",
        "actual-fill-aggregate",
        "actual-client-aggregate",
        "93",
        "1",
        "-0.01",
        1_780_000_000_100,
    )
    .await;

    let orders = query_live_orders(&pool, 10, "simulated", run_id)
        .await
        .expect("orders should query");

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["order_id"].as_str(), Some("algo-fill-aggregate"));
    assert_eq!(
        orders[0]["actual_order_id"].as_str(),
        Some("actual-fill-aggregate")
    );
    assert_eq!(orders[0]["fill_count"].as_i64(), Some(1));
    assert_eq!(orders[0]["filled_size"].as_f64(), Some(1.0));
    assert_eq!(orders[0]["remaining_size"].as_f64(), Some(0.0));
    assert_eq!(orders[0]["avg_fill_price"].as_f64(), Some(93.0));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_rejects_dirty_matching_fill_timestamp() {
    let db_path = temp_db_path("live_orders_dirty_matching_fill_ts");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-dirty-fill-ts";
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        1.0,
        100.0,
        "open_position",
        "partially_filled",
        true,
        "submitted",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
        "order-dirty-fill-ts",
        "client-dirty-fill-ts",
    )
    .await
    .expect("exchange order should insert");
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (
          'trade-dirty-ts', 'ETH-USDT-SWAP', 'ETH', 'buy', '100', '1',
          '-0.01', 'USDT', 'bad-ts', 'simulated', 'okx_private_ws',
          'order-dirty-fill-ts', 'client-dirty-fill-ts', 'arrival_storage_test',
          'run-dirty-fill-ts'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty local fill should insert");

    let error = query_live_orders(&pool, 10, "simulated", run_id)
        .await
        .expect_err("dirty matching fill timestamp should fail fast");

    assert!(error.to_string().contains("ts"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_context_treats_canceled_partial_fill_as_recent_fill() {
    let db_path = temp_db_path("live_order_context_canceled_partial_fill");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-canceled-partial-fill";
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "market",
        3.0,
        100.0,
        "close_position",
        "canceled",
        true,
        "OKX order canceled after partial fill",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
        "order-canceled-partial",
        "client-canceled-partial",
    )
    .await
    .expect("exchange order should insert");
    insert_local_fill_for_test(
        &pool,
        "trade-canceled-partial",
        "order-canceled-partial",
        "client-canceled-partial",
        "100",
        "1",
        "-0.01",
        1_780_000_000_100,
    )
    .await;

    let status = crate::live_strategy::LiveStrategyStatus {
        run_id: run_id.to_string(),
        mode: "simulated".to_string(),
        total_orders: 1,
        successful_orders: 1,
        failed_orders: 0,
        ..crate::live_strategy::LiveStrategyStatus::default()
    };
    let context = query_live_order_context(&pool, &status)
        .await
        .expect("order context should query");

    assert_eq!(context["recent_fills"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        context["recent_rejections"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        context["recent_fills"][0]["status"].as_str(),
        Some("canceled")
    );
    assert_eq!(
        context["recent_fills"][0]["filled_size"].as_f64(),
        Some(1.0)
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_context_marks_planned_exit_fill_as_close_position() {
    let db_path = temp_db_path("planned_exit_fill_close_position_action");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-planned-exit-fill";

    insert_live_order(
        &pool,
        &config,
        "sell",
        1.0,
        101.0,
        "close_position",
        "filled",
        true,
        "OKX order filled",
        run_id,
        1_780_000_900_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("planned exit fill should insert");

    let status = crate::live_strategy::LiveStrategyStatus {
        run_id: run_id.to_string(),
        mode: "simulated".to_string(),
        total_orders: 1,
        successful_orders: 1,
        ..crate::live_strategy::LiveStrategyStatus::default()
    };
    let context = query_live_order_context(&pool, &status)
        .await
        .expect("order context should query");

    assert_eq!(context["recent_fills"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        context["recent_fills"][0]["action"].as_str(),
        Some("close_position")
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

async fn insert_local_fill_for_test(
    pool: &sqlx::SqlitePool,
    trade_id: &str,
    order_id: &str,
    client_order_id: &str,
    fill_px: &str,
    fill_sz: &str,
    fee: &str,
    ts: i64,
) {
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (?, 'ETH-USDT-SWAP', 'ETH', 'buy', ?, ?, ?, 'USDT',
                  ?, 'simulated', 'okx_private_ws', ?, ?, 'arrival_storage_test',
                  'run-fill-aggregate')
        "#,
    )
    .bind(trade_id)
    .bind(fill_px)
    .bind(fill_sz)
    .bind(fee)
    .bind(ts)
    .bind(order_id)
    .bind(client_order_id)
    .execute(pool)
    .await
    .expect("local fill should insert");
}

async fn insert_order_sync_candidate_row(
    pool: &sqlx::SqlitePool,
    inst_type: &str,
    created_at: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (
          'sync_candidate_strategy', 'Sync Candidate Strategy',
          'ETH-USDT-SWAP', 'ETH-USDT-SWAP', ?, 'buy', 'market',
          1.0, 100.0, 'ord-sync-dirty', 'cl-sync-dirty', 'submitted',
          'open_position', '', 'simulated', 0, 'run-sync-dirty',
          1780000000000, ?
        )
        "#,
    )
    .bind(inst_type)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("sync candidate row should insert");
}

#[tokio::test]
async fn query_live_orders_rejects_dirty_order_economics() {
    let db_path = temp_db_path("query_dirty_order_economics");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (
          'strategy', 'Strategy', 'BTC-USDT-SWAP', 'BTC-USDT-SWAP',
          'buy', 'market', 'bad-size', 'bad-price', 'ord-dirty', 'cl-dirty',
          'filled', 'buy', '', 'simulated', 1, 'run-dirty-order',
          1780000000000, '2026-05-28T00:00:00Z'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty legacy order row should insert");

    let error = query_live_orders(&pool, 10, "simulated", "run-dirty-order")
        .await
        .expect_err("dirty order economics should fail fast");

    assert!(error.to_string().contains("size"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_rejects_dirty_order_timestamp() {
    let db_path = temp_db_path("query_dirty_order_timestamp");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (
          'strategy', 'Strategy', 'BTC-USDT-SWAP', 'BTC-USDT-SWAP',
          'buy', 'market', 1.0, 100.0, 'ord-dirty-ts', 'cl-dirty-ts',
          'filled', 'open_position', '', 'simulated', 1, 'run-dirty-order-ts',
          NULL, '2026-05-28T00:00:00Z'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty legacy order row should insert");

    let error = query_live_orders(&pool, 10, "simulated", "run-dirty-order-ts")
        .await
        .expect_err("dirty order timestamp should fail fast");

    assert!(error.to_string().contains("action_timestamp"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_rejects_dirty_success_flag() {
    let db_path = temp_db_path("query_dirty_order_success");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (
          'strategy', 'Strategy', 'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'SWAP',
          'buy', 'market', 1.0, 100.0, 'ord-dirty-success', 'cl-dirty-success',
          'filled', 'open_position', '', 'simulated', 2, 'run-dirty-order-success',
          1780000000000, '2026-05-28T00:00:00Z'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty success flag row should insert");

    let error = query_live_orders(&pool, 10, "simulated", "run-dirty-order-success")
        .await
        .expect_err("dirty success flag should fail fast");

    assert!(error.to_string().contains("success"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_sync_candidates_rejects_missing_inst_type() {
    let db_path = temp_db_path("query_sync_candidate_missing_inst_type");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    insert_order_sync_candidate_row(&pool, "", "2026-05-28T00:00:00Z").await;

    let error = query_live_order_sync_candidates(&pool, "simulated", "sync_candidate_strategy", 10)
        .await
        .expect_err("missing inst_type should fail fast");
    assert!(
        error.to_string().contains("inst_type"),
        "unexpected error: {error}"
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_sync_candidates_rejects_dirty_created_at() {
    let db_path = temp_db_path("query_sync_candidate_dirty_created_at");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    insert_order_sync_candidate_row(&pool, "SWAP", "not-a-timestamp").await;

    let error = query_live_order_sync_candidates(&pool, "simulated", "sync_candidate_strategy", 10)
        .await
        .expect_err("dirty created_at should fail fast");
    assert!(
        error.to_string().contains("created_at_ms"),
        "unexpected error: {error}"
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_fill_sync_scopes_rejects_missing_inst_type() {
    let db_path = temp_db_path("query_fill_scope_missing_inst_type");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    insert_order_sync_candidate_row(&pool, "", "2026-05-28T00:00:00Z").await;

    let error = query_live_fill_sync_scopes(&pool, "simulated", "sync_candidate_strategy", 10)
        .await
        .expect_err("missing inst_type should fail fast");
    assert!(
        error.to_string().contains("inst_type"),
        "unexpected error: {error}"
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_orders_uses_scope_recent_indexes() {
    let db_path = temp_db_path("live_orders_scope_recent_plan");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    let scoped_plan = sqlx::query(
        r#"
        EXPLAIN QUERY PLAN
        SELECT id FROM live_order_records
        WHERE mode = ? AND run_id = ?
        ORDER BY created_at DESC, id DESC
        LIMIT ?
        "#,
    )
    .bind("simulated")
    .bind("run-plan")
    .bind(300)
    .fetch_all(&pool)
    .await
    .expect("explain scoped live order query")
    .into_iter()
    .map(|row| row.try_get::<String, _>("detail").expect("plan detail"))
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        scoped_plan.contains("idx_live_orders_mode_run_recent"),
        "expected scoped live order query to use idx_live_orders_mode_run_recent, got:\n{scoped_plan}"
    );

    let mode_plan = sqlx::query(
        r#"
        EXPLAIN QUERY PLAN
        SELECT id FROM live_order_records
        WHERE mode = ?
        ORDER BY created_at DESC, id DESC
        LIMIT ?
        "#,
    )
    .bind("simulated")
    .bind(300)
    .fetch_all(&pool)
    .await
    .expect("explain mode live order query")
    .into_iter()
    .map(|row| row.try_get::<String, _>("detail").unwrap_or_default())
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        mode_plan.contains("idx_live_orders_mode_recent"),
        "expected mode live order query to use idx_live_orders_mode_recent, got:\n{mode_plan}"
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_context_keeps_algo_order_until_exchange_update() {
    let db_path = temp_db_path("order_context_algo_until_exchange_update");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-order-context-risk-close";

    insert_live_order(
        &pool,
        &config,
        "buy",
        1.0,
        100.0,
        "open_position",
        "filled",
        true,
        "entry",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("entry fill should insert");
    insert_live_order_with_type(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "protective stop",
        run_id,
        1_780_000_000_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("open risk order should insert");
    insert_live_order(
        &pool,
        &config,
        "sell",
        1.0,
        94.0,
        "close_position",
        "filled",
        true,
        "stop loss filled",
        run_id,
        1_780_000_060_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("close fill should insert");

    let status = crate::live_strategy::LiveStrategyStatus {
        run_id: run_id.to_string(),
        mode: "simulated".to_string(),
        total_orders: 3,
        successful_orders: 3,
        failed_orders: 0,
        ..crate::live_strategy::LiveStrategyStatus::default()
    };
    let context = query_live_order_context(&pool, &status)
        .await
        .expect("order context should query");

    assert_eq!(context["open"].as_array().map(Vec::len), Some(1));
    assert_eq!(context["recent_fills"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        context["recent_rejections"].as_array().map(Vec::len),
        Some(0)
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_order_context_keeps_algo_order_without_exchange_cancel() {
    let db_path = temp_db_path("order_context_algo_without_exchange_cancel");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let run_id = "run-order-context-unknown-risk-open-ts";

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (
          'arrival_storage_test', 'Arrival Storage Test', 'ETH-USDT-SWAP', 'ETH-USDT-SWAP', 'SWAP',
          'sell', 'stop_market', 1.0, 94.0, 'risk-open-unknown-ts', 'cl-risk-open-unknown-ts',
          'algo_live', 'place_risk_order', 'protective stop', 'simulated', 1,
          ?, 1780000000000, '2026-05-28T00:00:00Z'
        )
        "#,
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("open algo order should insert");

    insert_live_order(
        &pool,
        &config,
        "sell",
        1.0,
        94.0,
        "close_position",
        "filled",
        true,
        "stop loss filled",
        run_id,
        1_780_000_060_000,
        ArrivalQuote::default(),
    )
    .await
    .expect("close fill should insert");

    let status = crate::live_strategy::LiveStrategyStatus {
        run_id: run_id.to_string(),
        mode: "simulated".to_string(),
        total_orders: 2,
        successful_orders: 2,
        failed_orders: 0,
        ..crate::live_strategy::LiveStrategyStatus::default()
    };
    let context = query_live_order_context(&pool, &status)
        .await
        .expect("order context should query");

    assert_eq!(
        context["open"].as_array().map(Vec::len),
        Some(1),
        "algo order must stay open until OKX reports cancellation or completion"
    );
    assert_eq!(
        context["recent_rejections"].as_array().map(Vec::len),
        Some(0)
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
