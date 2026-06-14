use super::*;

#[tokio::test]
async fn private_ws_fill_event_upserts_local_fill_with_live_order_evidence() {
    let db_path = temp_db_path("private_ws_fill_event");
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
        "run-ws-entry",
        1_780_000_000_000,
        ArrivalQuote {
            ts_ms: Some(1_780_000_000_000),
            mid_px: Some(100.25),
            bid_px: Some(100.0),
            ask_px: Some(100.5),
        },
        "ws-entry-order",
        "wsentryclient",
    )
    .await
    .expect("entry order should insert");

    let outcome = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-trade-1",
            "ord_id": "ws-entry-order",
            "cl_ord_id": "wsentryclient",
            "inst_id": "BTC-USDT-SWAP",
            "side": "buy",
            "fill_px": 100.5,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000123_i64,
            "source": "okx_private_ws",
            "raw": {
                "tradeId": "ws-trade-1",
                "ordId": "ws-entry-order",
                "clOrdId": "wsentryclient",
                "instId": "BTC-USDT-SWAP",
                "side": "buy",
                "fillPx": "100.5",
                "fillSz": "1",
                "fee": "-0.01",
                "feeCcy": "USDT",
                "ts": "1780000000123"
            }
        }),
    )
    .await
    .expect("ws fill event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 0);
    let fill = sqlx::query(
        r#"
        SELECT strategy_id, run_id, fill_px, fill_sz, source, arrival_mid_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("ws-trade-1")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("fill should be stored");
    assert_eq!(fill.get::<String, _>("strategy_id"), config.strategy_id);
    assert_eq!(fill.get::<String, _>("run_id"), "run-ws-entry");
    assert_eq!(fill.get::<String, _>("fill_px"), "100.5");
    assert_eq!(
        fill.get::<String, _>("fill_sz")
            .parse::<f64>()
            .expect("fill size should parse"),
        1.0
    );
    assert_eq!(fill.get::<String, _>("source"), "okx_private_ws");
    assert_eq!(fill.get::<Option<f64>, _>("arrival_mid_px"), Some(100.25));
    let order_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("ws-entry-order")
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(order_status, "filled");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_fill_event_is_scoped_by_symbol_when_client_order_id_collides() {
    let db_path = temp_db_path("private_ws_fill_symbol_scope");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let mut eth_config = config.clone();
    eth_config.symbol = "ETH-USDT-SWAP".to_string();

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
        "btc submitted",
        "run-btc-fill-scoped",
        1_780_000_000_000,
        ArrivalQuote {
            ts_ms: Some(1_780_000_000_000),
            mid_px: Some(100.25),
            bid_px: Some(100.0),
            ask_px: Some(100.5),
        },
        "btc-fill-scoped-order",
        "sharedfillclient",
    )
    .await
    .expect("btc order should insert");
    insert_live_exchange_order(
        &pool,
        &eth_config,
        "buy",
        "market",
        1.0,
        200.0,
        "open_position",
        "submitted",
        true,
        "eth submitted",
        "run-eth-fill-scoped",
        1_780_000_000_001,
        ArrivalQuote {
            ts_ms: Some(1_780_000_000_001),
            mid_px: Some(200.25),
            bid_px: Some(200.0),
            ask_px: Some(200.5),
        },
        "eth-fill-scoped-order",
        "sharedfillclient",
    )
    .await
    .expect("eth order should insert");

    let outcome = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "btc-fill-scoped-trade",
            "ord_id": "btc-fill-scoped-order",
            "cl_ord_id": "sharedfillclient",
            "inst_id": "BTC-USDT-SWAP",
            "side": "buy",
            "fill_px": 100.5,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000123_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("scoped ws fill event should persist");

    assert_eq!(outcome.order_changed, 1);
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedfillclient")
    .fetch_all(&pool)
    .await
    .expect("orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "filled");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "submitted");

    let fill = sqlx::query(
        r#"
        SELECT strategy_id, run_id, arrival_mid_px
        FROM local_fills
        WHERE trade_id = ? AND mode = ?
        "#,
    )
    .bind("btc-fill-scoped-trade")
    .bind("simulated")
    .fetch_one(&pool)
    .await
    .expect("fill should exist");
    assert_eq!(fill.get::<String, _>("strategy_id"), config.strategy_id);
    assert_eq!(fill.get::<String, _>("run_id"), "run-btc-fill-scoped");
    assert_eq!(fill.get::<Option<f64>, _>("arrival_mid_px"), Some(100.25));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn fill_aggregate_rejects_duplicate_active_order_identity_matches() {
    let db_path = temp_db_path("fill_aggregate_duplicate_order_identity");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp
        ) VALUES
          (
            'order_sync_runtime_test', 'Order Sync Runtime Test',
            'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'SWAP', 'buy', 'market',
            1.0, 100.0, 'duplicate-fill-order', 'duplicatefillclient',
            'submitted', 'open_position', 'first submitted', 'simulated', 1,
            'run-duplicate-fill-1', 1780000000000
          ),
          (
            'order_sync_runtime_test', 'Order Sync Runtime Test',
            'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'SWAP', 'buy', 'market',
            1.0, 100.0, 'duplicate-fill-order', 'duplicatefillclient',
            'submitted', 'open_position', 'second submitted', 'simulated', 1,
            'run-duplicate-fill-2', 1780000000001
          )
        "#,
    )
    .execute(&pool)
    .await
    .expect("legacy duplicate local orders should insert for guard test");
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (
          'duplicate-fill-trade', 'BTC-USDT-SWAP', 'USDT', 'buy',
          '100.5', '1', '-0.01', 'USDT', 1780000000123,
          'simulated', 'unit_test', 'duplicate-fill-order', 'duplicatefillclient',
          'order_sync_runtime_test', 'run-duplicate-fill-1'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("local fill should insert");

    let error = apply_fill_aggregate_to_live_order(
        &pool,
        "simulated",
        "BTC-USDT-SWAP",
        "duplicate-fill-order",
        "duplicatefillclient",
    )
    .await
    .expect_err("duplicate active order identity matches must be rejected");
    assert!(error.to_string().contains("多条活动订单记录"));

    let filled_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM live_order_records
        WHERE order_id = ? AND status = 'filled'
        "#,
    )
    .bind("duplicate-fill-order")
    .fetch_one(&pool)
    .await
    .expect("filled count should query");
    assert_eq!(filled_count, 0);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn fill_aggregate_rejects_dirty_local_order_size() {
    let db_path = temp_db_path("fill_aggregate_dirty_order_size");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp
        ) VALUES (
          'order_sync_runtime_test', 'Order Sync Runtime Test',
          'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'SWAP', 'buy', 'market',
          'bad-size', 100.0, 'dirty-size-fill-order', 'dirtysizefillclient',
          'submitted', 'open_position', 'submitted', 'simulated', 1,
          'run-dirty-size-fill', 1780000000000
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty local order should insert");
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (
          'dirty-size-fill-trade', 'BTC-USDT-SWAP', 'USDT', 'buy',
          '100.5', '1', '-0.01', 'USDT', 1780000000123,
          'simulated', 'unit_test', 'dirty-size-fill-order', 'dirtysizefillclient',
          'order_sync_runtime_test', 'run-dirty-size-fill'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("local fill should insert");

    let error = apply_fill_aggregate_to_live_order(
        &pool,
        "simulated",
        "BTC-USDT-SWAP",
        "dirty-size-fill-order",
        "dirtysizefillclient",
    )
    .await
    .expect_err("dirty local order size should fail fast");
    assert!(error.to_string().contains("size"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn fill_aggregate_rejects_missing_local_order_action() {
    let db_path = temp_db_path("fill_aggregate_missing_order_action");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");

    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp
        ) VALUES (
          'order_sync_runtime_test', 'Order Sync Runtime Test',
          'BTC-USDT-SWAP', 'BTC-USDT-SWAP', 'SWAP', 'buy', 'market',
          1.0, 100.0, 'missing-action-fill-order', 'missingactionfillclient',
          'submitted', NULL, 'submitted', 'simulated', 1,
          'run-missing-action-fill', 1780000000000
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("dirty local order should insert");
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (
          'missing-action-fill-trade', 'BTC-USDT-SWAP', 'USDT', 'buy',
          '100.5', '1', '-0.01', 'USDT', 1780000000123,
          'simulated', 'unit_test', 'missing-action-fill-order', 'missingactionfillclient',
          'order_sync_runtime_test', 'run-missing-action-fill'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("local fill should insert");

    let error = apply_fill_aggregate_to_live_order(
        &pool,
        "simulated",
        "BTC-USDT-SWAP",
        "missing-action-fill-order",
        "missingactionfillclient",
    )
    .await
    .expect_err("missing local order action should fail fast");
    assert!(error.to_string().contains("action"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_fill_event_updates_order_management_request_states() {
    let db_path = temp_db_path("private_ws_fill_order_management_requests");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "market",
        2.0,
        100.0,
        "open_position",
        "cancel_requested",
        true,
        "cancel requested",
        "run-ws-cancel-requested",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "ws-cancel-requested-order",
        "wscancelrequested",
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
        "run-ws-modify-requested",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "ws-modify-requested-order",
        "wsmodifyrequested",
    )
    .await
    .expect("modify requested order should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_modify_requested",
        true,
        "algo modify requested",
        "run-ws-algo-modify-requested",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-modify-requested-order",
        "clalgomodifyrequested",
    )
    .await
    .expect("algo modify requested order should insert");
    sqlx::query(
        r#"
        UPDATE live_order_records
        SET actual_order_id = ?, actual_client_order_id = ?
        WHERE order_id = ?
        "#,
    )
    .bind("actual-algo-modify-order")
    .bind("actualclalgomodify")
    .bind("algo-modify-requested-order")
    .execute(&pool)
    .await
    .expect("actual algo identity should update");

    let partial = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-cancel-requested-partial",
            "ord_id": "ws-cancel-requested-order",
            "cl_ord_id": "wscancelrequested",
            "inst_id": "BTC-USDT-SWAP",
            "side": "buy",
            "fill_px": 100.1,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000123_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("partial fill should update cancel requested order");
    assert_eq!(partial.order_changed, 1);
    assert_eq!(partial.planned_exit_changed, 0);

    let complete = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-modify-requested-complete",
            "ord_id": "ws-modify-requested-order",
            "cl_ord_id": "wsmodifyrequested",
            "inst_id": "BTC-USDT-SWAP",
            "side": "buy",
            "fill_px": 100.2,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000456_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("complete fill should update modify requested order");
    assert_eq!(complete.order_changed, 1);
    assert_eq!(complete.planned_exit_changed, 0);

    let algo_complete = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-algo-modify-requested-complete",
            "ord_id": "actual-algo-modify-order",
            "cl_ord_id": "actualclalgomodify",
            "inst_id": "BTC-USDT-SWAP",
            "side": "sell",
            "fill_px": 93.8,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000789_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("complete fill should update algo modify requested order");
    assert_eq!(algo_complete.order_changed, 1);
    assert_eq!(algo_complete.planned_exit_changed, 0);

    let cancel_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("ws-cancel-requested-order")
            .fetch_one(&pool)
            .await
            .expect("cancel requested order should exist");
    assert_eq!(cancel_status, "partially_filled");

    let modify_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("ws-modify-requested-order")
            .fetch_one(&pool)
            .await
            .expect("modify requested order should exist");
    assert_eq!(modify_status, "filled");

    let algo_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("algo-modify-requested-order")
            .fetch_one(&pool)
            .await
            .expect("algo modify requested order should exist");
    assert_eq!(algo_status, "algo_effective");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_fill_event_completes_planned_exit_without_order_terminal_event() {
    let db_path = temp_db_path("private_ws_fill_completes_planned_exit");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
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
        2.0,
        101.0,
        "close_position",
        "submitted",
        true,
        "submitted",
        "run-ws-exit",
        planned_exit.timestamp,
        ArrivalQuote::default(),
        "ws-exit-fill-order",
        "wsexitfillclient",
    )
    .await
    .expect("exit order should insert");
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-ws-exit",
        "ws-exit-fill-order",
        "wsexitfillclient",
    )
    .await
    .expect("plan should be exit_submitted");

    let partial = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-exit-fill-partial",
            "ord_id": "ws-exit-fill-order",
            "cl_ord_id": "wsexitfillclient",
            "inst_id": "BTC-USDT-SWAP",
            "side": "sell",
            "fill_px": 101.0,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000123_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("partial fill should persist");

    assert_eq!(partial.order_changed, 1);
    assert_eq!(partial.planned_exit_changed, 0);
    let partial_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("ws-exit-fill-order")
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(partial_status, "partially_filled");

    let complete = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "ws-exit-fill-complete",
            "ord_id": "ws-exit-fill-order",
            "cl_ord_id": "wsexitfillclient",
            "inst_id": "BTC-USDT-SWAP",
            "side": "sell",
            "fill_px": 101.2,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1780000000456_i64,
            "source": "okx_private_ws"
        }),
    )
    .await
    .expect("complete fill should persist");

    assert_eq!(complete.order_changed, 1);
    assert_eq!(complete.planned_exit_changed, 1);
    let row =
        sqlx::query("SELECT status, error_message FROM live_order_records WHERE order_id = ?")
            .bind("ws-exit-fill-order")
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "filled");
    assert!(row
        .get::<String, _>("error_message")
        .contains("fill event completed order"));
    let plan_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(plan.id)
            .fetch_one(&pool)
            .await
            .expect("plan should exist");
    assert_eq!(plan_status, "exit_filled");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn rest_fill_sync_completes_planned_exit_without_order_terminal_event() {
    let (base_url, stop_server) = start_order_detail_mock_server().await;
    let db_path = temp_db_path("rest_fill_completes_planned_exit");
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
        "run-rest-exit",
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
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-rest-exit",
        "exit-order",
        "exitclientorder",
    )
    .await
    .expect("plan should be exit_submitted");

    runtime
        .sync_exchange_fills("run-rest-exit", &config, &pool, &private_client)
        .await;

    let row =
        sqlx::query("SELECT status, error_message FROM live_order_records WHERE order_id = ?")
            .bind("exit-order")
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "filled");
    assert!(row
        .get::<String, _>("error_message")
        .contains("fill event completed order"));
    let plan_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(plan.id)
            .fetch_one(&pool)
            .await
            .expect("plan should exist");
    assert_eq!(plan_status, "exit_filled");
    let fill_source: String =
        sqlx::query_scalar("SELECT source FROM local_fills WHERE trade_id = ? AND mode = ?")
            .bind("trade-exit")
            .bind("simulated")
            .fetch_one(&pool)
            .await
            .expect("fill should be stored");
    assert_eq!(fill_source, "okx_fills_history");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
