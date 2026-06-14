use super::*;
use crate::live_strategy::order_sync::runtime::OrderNotFoundRejectionRequest;

#[tokio::test]
async fn exchange_order_sync_updates_local_order_and_planned_exit_terminal_state() {
    let (base_url, stop_server) = start_order_detail_mock_server().await;
    let db_path = temp_db_path("exchange_order_sync_terminal_plan");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let runtime = LiveStrategyRuntime::new();
    let private_client = OkxPrivateClient::new_with_proxy(
        base_url,
        ApiCredentials {
            api_key: "key".to_string(),
            secret_key: "secret".to_string(),
            passphrase: "pass".to_string(),
        },
        true,
        "direct",
    )
    .expect("private client");

    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "entry".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.1),
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let plan = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order",
        "entry-client-order",
    )
    .await
    .expect("plan should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "market",
        1.0,
        100.0,
        "close_position",
        "submitted",
        true,
        "submitted",
        "run-exit",
        planned_exit.timestamp,
        ArrivalQuote {
            ts_ms: Some(planned_exit.timestamp),
            mid_px: Some(100.25),
            bid_px: Some(100.0),
            ask_px: Some(100.5),
        },
        "exit-order",
        "exitclientorder",
    )
    .await
    .expect("exit order should insert");
    mark_live_planned_exit_submitted(&pool, plan.id, "run-exit", "exit-order", "exitclientorder")
        .await
        .expect("plan should be exit_submitted");

    runtime
        .sync_exchange_order_states("run-exit", &config, &pool, &private_client)
        .await;

    let order_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("exit-order")
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(order_status, "filled");
    let plan_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(plan.id)
            .fetch_one(&pool)
            .await
            .expect("plan should exist");
    assert_eq!(plan_status, "exit_filled");
    let fill = sqlx::query(
        r#"
        SELECT strategy_id, run_id, fill_px, fill_sz, fee, fee_ccy,
               arrival_ts, arrival_mid_px, arrival_bid_px, arrival_ask_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("trade-exit")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("fill should be stored");
    assert_eq!(fill.get::<String, _>("strategy_id"), config.strategy_id);
    assert_eq!(fill.get::<String, _>("run_id"), "run-exit");
    assert_eq!(fill.get::<String, _>("fill_px"), "100.5");
    assert_eq!(fill.get::<String, _>("fill_sz"), "1");
    assert_eq!(fill.get::<String, _>("fee"), "-0.01");
    assert_eq!(fill.get::<String, _>("fee_ccy"), "USDT");
    assert_eq!(
        fill.get::<Option<i64>, _>("arrival_ts"),
        Some(planned_exit.timestamp)
    );
    assert_eq!(fill.get::<Option<f64>, _>("arrival_mid_px"), Some(100.25));
    assert_eq!(fill.get::<Option<f64>, _>("arrival_bid_px"), Some(100.0));
    assert_eq!(fill.get::<Option<f64>, _>("arrival_ask_px"), Some(100.5));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn order_management_request_states_sync_from_order_history_when_active_lookup_is_missing() {
    let (base_url, stop_server) = start_order_not_found_with_history_mock_server().await;
    let db_path = temp_db_path("order_management_history_sync");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let runtime = LiveStrategyRuntime::new();
    let private_client = OkxPrivateClient::new_with_proxy(
        base_url,
        ApiCredentials {
            api_key: "key".to_string(),
            secret_key: "secret".to_string(),
            passphrase: "pass".to_string(),
        },
        true,
        "direct",
    )
    .expect("private client");

    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "cancel_requested",
        true,
        "cancel requested",
        "run-cancel-requested",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "cancel-history-order",
        "cancelhistoryclient",
    )
    .await
    .expect("cancel requested order should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        2.0,
        101.0,
        "open_position",
        "modify_requested",
        true,
        "modify requested",
        "run-modify-requested",
        1_780_000_001_000,
        ArrivalQuote::default(),
        "modify-history-order",
        "modifyhistoryclient",
    )
    .await
    .expect("modify requested order should insert");

    runtime
        .sync_exchange_order_states("run-history-sync", &config, &pool, &private_client)
        .await;

    let rows = sqlx::query(
        r#"
        SELECT order_id, status, success, error_message
        FROM live_order_records
        WHERE order_id IN (?, ?)
        ORDER BY order_id
        "#,
    )
    .bind("cancel-history-order")
    .bind("modify-history-order")
    .fetch_all(&pool)
    .await
    .expect("orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("order_id"), "cancel-history-order");
    assert_eq!(rows[0].get::<String, _>("status"), "canceled");
    assert_eq!(rows[0].get::<i64, _>("success"), 0);
    assert!(rows[0]
        .get::<String, _>("error_message")
        .contains("canceled without fill"));
    assert_eq!(rows[1].get::<String, _>("order_id"), "modify-history-order");
    assert_eq!(rows[1].get::<String, _>("status"), "filled");
    assert_eq!(rows[1].get::<i64, _>("success"), 1);
    assert!(rows[1]
        .get::<String, _>("error_message")
        .contains("avgPx=101.5"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn submit_unknown_entry_not_found_cancels_scheduled_planned_exit() {
    let db_path = temp_db_path("submit_unknown_entry_not_found_cancels_plan");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let runtime = LiveStrategyRuntime::new();
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "entry".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.1),
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let plan = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "",
        "entryunknownclient",
    )
    .await
    .expect("plan should insert");
    let row_id = insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        1.0,
        100.0,
        "open_position",
        "submit_unknown",
        true,
        "submit unknown",
        "run-entry",
        action_record.timestamp,
        ArrivalQuote::default(),
        "",
        "entryunknownclient",
    )
    .await
    .expect("entry order should insert");
    let candidate = LiveOrderSyncCandidate {
        id: row_id,
        symbol: config.symbol.clone(),
        inst_type: config.inst_type.clone(),
        order_id: String::new(),
        client_order_id: "entryunknownclient".to_string(),
        status: "submit_unknown".to_string(),
        created_at_ms: chrono::Utc::now().timestamp_millis() - 60_000,
    };

    let changed = runtime
        .mark_not_found_order_rejected(OrderNotFoundRejectionRequest {
            run_id: "run-sync",
            config: &config,
            db: &pool,
            candidate: &candidate,
            order_id: "",
            client_order_id: "entryunknownclient",
            error_message: "OKX private API error 51603: Order does not exist",
        })
        .await;

    assert!(changed);
    let order_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE id = ?")
            .bind(row_id)
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(order_status, "rejected");
    let plan_row = sqlx::query("SELECT status, last_error FROM live_execution_plans WHERE id = ?")
        .bind(plan.id)
        .fetch_one(&pool)
        .await
        .expect("plan should exist");
    assert_eq!(plan_row.get::<String, _>("status"), "cancelled");
    assert!(plan_row
        .get::<String, _>("last_error")
        .contains("Order does not exist"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
