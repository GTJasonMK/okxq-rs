use super::*;

#[tokio::test]
async fn private_ws_events_require_explicit_mode_before_persisting() {
    let db_path = temp_db_path("private_ws_requires_mode");
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
        "run-missing-mode",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "ws-missing-mode",
        "wsmissingmode",
    )
    .await
    .expect("local order should insert");

    let error = persist_private_order_event(
        &pool,
        &json!({
            "ord_id": "ws-missing-mode",
            "cl_ord_id": "wsmissingmode",
            "state": "filled",
            "raw": {
                "ordId": "ws-missing-mode",
                "clOrdId": "wsmissingmode",
                "state": "filled",
                "accFillSz": "1",
                "avgPx": "100.5"
            }
        }),
    )
    .await
    .expect_err("missing mode must not default to simulated");
    assert!(error.to_string().contains("缺少 mode"));

    let row = sqlx::query("SELECT status FROM live_order_records WHERE order_id = ?")
        .bind("ws-missing-mode")
        .fetch_one(&pool)
        .await
        .expect("local order should still exist");
    assert_eq!(row.get::<String, _>("status"), "submitted");

    let algo_error = persist_private_algo_order_event(
        &pool,
        &json!({
            "algo_id": "algo-missing-mode",
            "state": "effective",
            "raw": {
                "algoId": "algo-missing-mode",
                "state": "effective"
            }
        }),
    )
    .await
    .expect_err("missing algo mode must not default to simulated");
    assert!(algo_error.to_string().contains("缺少 mode"));

    let fill_error = persist_private_fill_event(
        &pool,
        &json!({
            "trade_id": "trade-missing-mode",
            "ord_id": "ws-missing-mode",
            "inst_id": "BTC-USDT-SWAP",
            "fill_px": 100.5,
            "fill_sz": 1.0,
            "ts": 1_780_000_000_100_i64
        }),
    )
    .await
    .expect_err("missing fill mode must not default to simulated");
    assert!(fill_error.to_string().contains("缺少 mode"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_events_reject_removed_mode_aliases_before_persisting() {
    let db_path = temp_db_path("private_ws_rejects_mode_aliases");
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
        "run-paper-mode",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "ws-paper-mode",
        "wspapermode",
    )
    .await
    .expect("local order should insert");

    for mode in ["paper", "demo", "simulation"] {
        let error = persist_private_order_event(
            &pool,
            &json!({
                "mode": mode,
                "ord_id": "ws-paper-mode",
                "cl_ord_id": "wspapermode",
                "state": "filled",
                "raw": {
                    "ordId": "ws-paper-mode",
                    "clOrdId": "wspapermode",
                    "state": "filled",
                    "accFillSz": "1",
                    "avgPx": "100.5"
                }
            }),
        )
        .await
        .expect_err("removed mode aliases must not be normalized into simulated");
        assert!(
            error.to_string().contains(&format!("mode={mode} 不受支持")),
            "unexpected error for mode={mode}: {error}"
        );
    }

    let row = sqlx::query("SELECT status FROM live_order_records WHERE order_id = ?")
        .bind("ws-paper-mode")
        .fetch_one(&pool)
        .await
        .expect("local order should still exist");
    assert_eq!(row.get::<String, _>("status"), "submitted");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_algo_order_event_updates_standalone_algo_order_state() {
    let db_path = temp_db_path("private_ws_algo_order_event");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "algo live",
        "run-algo-ws",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-ws-1",
        "clalgows1",
    )
    .await
    .expect("algo order should insert");

    let outcome = persist_private_algo_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "algo_id": "algo-ws-1",
            "algo_cl_ord_id": "clalgows1",
            "state": "effective",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "algoId": "algo-ws-1",
                "algoClOrdId": "clalgows1",
                "state": "effective",
                "actualSz": "1"
            }
        }),
    )
    .await
    .expect("algo order event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 0);
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("algo-ws-1")
    .fetch_one(&pool)
    .await
    .expect("algo order should exist");
    assert_eq!(row.get::<String, _>("status"), "algo_effective");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row.get::<String, _>("error_message").contains("actualSz=1"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_order_event_is_scoped_by_symbol_when_client_order_id_collides() {
    let db_path = temp_db_path("private_ws_order_symbol_scope");
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
        "run-btc-scoped",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "btc-scoped-order",
        "sharedscopedclient",
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
        "run-eth-scoped",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "eth-scoped-order",
        "sharedscopedclient",
    )
    .await
    .expect("eth order should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "btc-scoped-order",
            "cl_ord_id": "sharedscopedclient",
            "state": "filled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "btc-scoped-order",
                "clOrdId": "sharedscopedclient",
                "state": "filled",
                "accFillSz": "1",
                "avgPx": "100.5"
            }
        }),
    )
    .await
    .expect("scoped ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedscopedclient")
    .fetch_all(&pool)
    .await
    .expect("orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "filled");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "submitted");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_algo_order_event_is_scoped_by_symbol_when_client_order_id_collides() {
    let db_path = temp_db_path("private_ws_algo_symbol_scope");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let mut eth_config = config.clone();
    eth_config.symbol = "ETH-USDT-SWAP".to_string();

    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "btc algo live",
        "run-btc-algo-scoped",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "btc-scoped-algo",
        "sharedscopedalgo",
    )
    .await
    .expect("btc algo order should insert");
    insert_live_exchange_order(
        &pool,
        &eth_config,
        "sell",
        "stop_market",
        1.0,
        194.0,
        "place_risk_order",
        "algo_live",
        true,
        "eth algo live",
        "run-eth-algo-scoped",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "eth-scoped-algo",
        "sharedscopedalgo",
    )
    .await
    .expect("eth algo order should insert");

    let outcome = persist_private_algo_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "algo_id": "btc-scoped-algo",
            "algo_cl_ord_id": "sharedscopedalgo",
            "state": "effective",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "algoId": "btc-scoped-algo",
                "algoClOrdId": "sharedscopedalgo",
                "state": "effective",
                "actualSz": "1"
            }
        }),
    )
    .await
    .expect("scoped ws algo event should persist");

    assert_eq!(outcome.order_changed, 1);
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedscopedalgo")
    .fetch_all(&pool)
    .await
    .expect("orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "algo_effective");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "algo_live");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_linked_algo_actual_order_event_preserves_algo_identity_and_attributes_fills() {
    let db_path = temp_db_path("private_ws_linked_algo_actual_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "algo live",
        "run-linked-algo",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-linked-1",
        "clalgolinked1",
    )
    .await
    .expect("algo order should insert");

    let order_outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "actual-order-1",
            "cl_ord_id": "actual-client-1",
            "state": "filled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "actual-order-1",
                "clOrdId": "actual-client-1",
                "algoId": "algo-linked-1",
                "algoClOrdId": "clalgolinked1",
                "state": "filled",
                "accFillSz": "1",
                "avgPx": "93.5"
            }
        }),
    )
    .await
    .expect("linked algo order event should persist");
    assert_eq!(order_outcome.order_changed, 1);

    let fill_outcome = persist_private_fill_event(
        &pool,
        &json!({
            "mode": "simulated",
            "trade_id": "trade-linked-1",
            "ord_id": "actual-order-1",
            "cl_ord_id": "actual-client-1",
            "inst_id": "BTC-USDT-SWAP",
            "side": "sell",
            "fill_px": 93.5,
            "fill_sz": 1.0,
            "fee": -0.01,
            "fee_ccy": "USDT",
            "ts": 1_780_000_000_100_i64,
            "raw": {
                "tradeId": "trade-linked-1",
                "ordId": "actual-order-1",
                "clOrdId": "actual-client-1",
                "instId": "BTC-USDT-SWAP",
                "side": "sell",
                "fillPx": "93.5",
                "fillSz": "1",
                "fee": "-0.01",
                "feeCcy": "USDT",
                "ts": "1780000000100"
            }
        }),
    )
    .await
    .expect("linked algo fill should persist");
    assert_eq!(fill_outcome.order_changed, 1);

    let order = sqlx::query(
        r#"
        SELECT order_id, client_order_id, actual_order_id, actual_client_order_id,
               status, success, error_message
        FROM live_order_records
        WHERE order_id = ?
        "#,
    )
    .bind("algo-linked-1")
    .fetch_one(&pool)
    .await
    .expect("algo order should exist");
    assert_eq!(order.get::<String, _>("order_id"), "algo-linked-1");
    assert_eq!(order.get::<String, _>("client_order_id"), "clalgolinked1");
    assert_eq!(order.get::<String, _>("actual_order_id"), "actual-order-1");
    assert_eq!(
        order.get::<String, _>("actual_client_order_id"),
        "actual-client-1"
    );
    assert_eq!(order.get::<String, _>("status"), "algo_effective");
    assert_eq!(order.get::<i64, _>("success"), 1);
    assert!(order
        .get::<String, _>("error_message")
        .contains("protective algo actual order filled"));

    let fill =
        sqlx::query("SELECT strategy_id, run_id FROM local_fills WHERE trade_id = ? AND mode = ?")
            .bind("trade-linked-1")
            .bind("simulated")
            .fetch_one(&pool)
            .await
            .expect("fill should exist");
    assert_eq!(fill.get::<String, _>("strategy_id"), config.strategy_id);
    assert_eq!(fill.get::<String, _>("run_id"), "run-linked-algo");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_linked_algo_actual_order_event_matches_nested_algo_client_identity() {
    let db_path = temp_db_path("private_ws_linked_algo_nested_client");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitted",
        true,
        "attached algo submitted",
        "run-linked-nested-client",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "attachnestedclient",
    )
    .await
    .expect("attached algo order should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "actual-nested-order",
            "cl_ord_id": "actual-nested-client",
            "state": "filled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "actual-nested-order",
                "clOrdId": "actual-nested-client",
                "linkedAlgoOrd": {
                    "algoClOrdId": "attachnestedclient"
                },
                "state": "filled",
                "accFillSz": "1",
                "avgPx": "93.5"
            }
        }),
    )
    .await
    .expect("linked algo order event should persist through nested algoClOrdId");

    assert_eq!(outcome.order_changed, 1);
    let order = sqlx::query(
        r#"
        SELECT order_id, client_order_id, actual_order_id, actual_client_order_id,
               status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("attachnestedclient")
    .fetch_one(&pool)
    .await
    .expect("attached algo order should exist");
    assert_eq!(order.get::<String, _>("order_id"), "");
    assert_eq!(
        order.get::<String, _>("client_order_id"),
        "attachnestedclient"
    );
    assert_eq!(
        order.get::<String, _>("actual_order_id"),
        "actual-nested-order"
    );
    assert_eq!(
        order.get::<String, _>("actual_client_order_id"),
        "actual-nested-client"
    );
    assert_eq!(order.get::<String, _>("status"), "algo_effective");
    assert_eq!(order.get::<i64, _>("success"), 1);
    assert!(order
        .get::<String, _>("error_message")
        .contains("protective algo actual order filled"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_order_event_updates_local_order_and_planned_exit_state() {
    let db_path = temp_db_path("private_ws_order_event");
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
        1.0,
        100.0,
        "close_position",
        "submitted",
        true,
        "submitted",
        "run-ws-exit",
        planned_exit.timestamp,
        ArrivalQuote::default(),
        "ws-exit-order",
        "wsexitclient",
    )
    .await
    .expect("exit order should insert");
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-ws-exit",
        "ws-exit-order",
        "wsexitclient",
    )
    .await
    .expect("plan should be exit_submitted");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "ws-exit-order",
            "cl_ord_id": "wsexitclient",
            "state": "filled",
            "avg_px": 100.5,
            "fill_sz": 1.0,
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "ws-exit-order",
                "clOrdId": "wsexitclient",
                "state": "filled",
                "avgPx": "100.5",
                "accFillSz": "1"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 1);
    let order_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("ws-exit-order")
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

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_rejected_entry_order_cancels_scheduled_planned_exit() {
    let db_path = temp_db_path("private_ws_entry_rejected_cancels_plan");
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
        "entry-order-rejected",
        "entryclientrejected",
    )
    .await
    .expect("plan should insert");
    let other_plan = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-other",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-other",
        "entryclientother",
    )
    .await
    .expect("other plan should insert");
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
        "run-entry",
        action_record.timestamp,
        ArrivalQuote::default(),
        "entry-order-rejected",
        "entryclientrejected",
    )
    .await
    .expect("entry order should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "entry-order-rejected",
            "cl_ord_id": "entryclientrejected",
            "state": "rejected",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "entry-order-rejected",
                "clOrdId": "entryclientrejected",
                "state": "rejected",
                "sMsg": "unit test reject"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 1);
    let plan_row = sqlx::query("SELECT status, last_error FROM live_execution_plans WHERE id = ?")
        .bind(plan.id)
        .fetch_one(&pool)
        .await
        .expect("plan should exist");
    assert_eq!(plan_row.get::<String, _>("status"), "cancelled");
    assert!(plan_row
        .get::<String, _>("last_error")
        .contains("unit test reject"));
    let other_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(other_plan.id)
            .fetch_one(&pool)
            .await
            .expect("other plan should exist");
    assert_eq!(other_status, "scheduled");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_rejected_parent_order_marks_attached_algo_terminal() {
    let db_path = temp_db_path("private_ws_parent_rejected_marks_attached_algo");
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
        "run-attached-reject",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "parent-reject-order",
        "parentrejectclient",
    )
    .await
    .expect("parent order should insert");
    insert_live_attached_algo_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "algo_submitted",
        true,
        "attached algo submitted",
        "run-attached-reject",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "attachedrejectclient",
        "parent-reject-order",
        "parentrejectclient",
    )
    .await
    .expect("attached algo should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "parent-reject-order",
            "cl_ord_id": "parentrejectclient",
            "state": "rejected",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "parent-reject-order",
                "clOrdId": "parentrejectclient",
                "state": "rejected",
                "accFillSz": "0",
                "sMsg": "unit test rejected"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 2);
    let parent_status: String =
        sqlx::query_scalar("SELECT status FROM live_order_records WHERE order_id = ?")
            .bind("parent-reject-order")
            .fetch_one(&pool)
            .await
            .expect("parent order should exist");
    assert_eq!(parent_status, "rejected");
    let attached_row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("attachedrejectclient")
    .fetch_one(&pool)
    .await
    .expect("attached algo should exist");
    assert_eq!(attached_row.get::<String, _>("status"), "algo_failed");
    assert_eq!(attached_row.get::<i64, _>("success"), 0);
    assert!(attached_row
        .get::<String, _>("error_message")
        .contains("父订单未成交终止"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_canceled_partial_parent_order_keeps_attached_algo_pending() {
    let db_path = temp_db_path("private_ws_parent_partial_keeps_attached_algo");
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
        "run-attached-partial",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "parent-partial-order",
        "parentpartialclient",
    )
    .await
    .expect("parent order should insert");
    insert_live_attached_algo_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "algo_submitted",
        true,
        "attached algo submitted",
        "run-attached-partial",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "attachedpartialclient",
        "parent-partial-order",
        "parentpartialclient",
    )
    .await
    .expect("attached algo should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "parent-partial-order",
            "cl_ord_id": "parentpartialclient",
            "state": "canceled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "parent-partial-order",
                "clOrdId": "parentpartialclient",
                "state": "canceled",
                "accFillSz": "0.4"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    let attached_row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("attachedpartialclient")
    .fetch_one(&pool)
    .await
    .expect("attached algo should exist");
    assert_eq!(attached_row.get::<String, _>("status"), "algo_submitted");
    assert_eq!(attached_row.get::<i64, _>("success"), 1);
    assert_eq!(
        attached_row.get::<String, _>("error_message"),
        "attached algo submitted"
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_canceled_partial_entry_order_keeps_planned_exit_scheduled() {
    let db_path = temp_db_path("private_ws_entry_canceled_partial_keeps_plan");
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
        "entry-order-canceled-partial",
        "entryclientcanceledpartial",
    )
    .await
    .expect("plan should insert");
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
        "run-entry",
        action_record.timestamp,
        ArrivalQuote::default(),
        "entry-order-canceled-partial",
        "entryclientcanceledpartial",
    )
    .await
    .expect("entry order should insert");

    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "entry-order-canceled-partial",
            "cl_ord_id": "entryclientcanceledpartial",
            "state": "canceled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "entry-order-canceled-partial",
                "clOrdId": "entryclientcanceledpartial",
                "state": "canceled",
                "accFillSz": "0.4"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 0);
    let plan_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(plan.id)
            .fetch_one(&pool)
            .await
            .expect("plan should exist");
    assert_eq!(plan_status, "scheduled");
    let order_row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("entry-order-canceled-partial")
    .fetch_one(&pool)
    .await
    .expect("entry order should exist");
    assert_eq!(order_row.get::<String, _>("status"), "canceled");
    assert_eq!(order_row.get::<i64, _>("success"), 1);
    assert!(order_row
        .get::<String, _>("error_message")
        .contains("partial fill"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn private_ws_canceled_planned_exit_order_requeues_plan_for_retry() {
    let db_path = temp_db_path("private_ws_order_event_requeue");
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
        1.0,
        100.0,
        "close_position",
        "submitted",
        true,
        "submitted",
        "run-ws-exit",
        planned_exit.timestamp,
        ArrivalQuote::default(),
        "ws-exit-order-cancel",
        "wsexitcancel",
    )
    .await
    .expect("exit order should insert");
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-ws-exit",
        "ws-exit-order-cancel",
        "wsexitcancel",
    )
    .await
    .expect("plan should be exit_submitted");

    let retry_lower_bound = chrono::Utc::now().timestamp_millis();
    let outcome = persist_private_order_event(
        &pool,
        &json!({
            "mode": "simulated",
            "inst_id": "BTC-USDT-SWAP",
            "ord_id": "ws-exit-order-cancel",
            "cl_ord_id": "wsexitcancel",
            "state": "canceled",
            "raw": {
                "instId": "BTC-USDT-SWAP",
                "ordId": "ws-exit-order-cancel",
                "clOrdId": "wsexitcancel",
                "state": "canceled"
            }
        }),
    )
    .await
    .expect("ws order event should persist");

    assert_eq!(outcome.order_changed, 1);
    assert_eq!(outcome.planned_exit_changed, 1);
    let row = sqlx::query(
        "SELECT status, attempt_count, next_attempt_at FROM live_execution_plans WHERE id = ?",
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "scheduled");
    assert_eq!(row.get::<i64, _>("attempt_count"), 1);
    assert!(row.get::<i64, _>("next_attempt_at") >= retry_lower_bound);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
