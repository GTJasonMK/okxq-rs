use super::*;

#[tokio::test]
async fn standalone_risk_order_submits_okx_algo_order_and_records_local_state() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_standalone_risk_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "place_risk_order".to_string(),
        side: "sell".to_string(),
        price: 0.0,
        reason: "unit_test_standalone_stop".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop-market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protect_existing_long".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-standalone-risk-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::PlaceRiskOrder,
            "stop-market",
            Some("sell"),
            None,
            &[risk],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order-algo"))
        .expect("standalone risk order should submit OKX algo order");
    let body = request_json_body(algo_request).expect("algo order should have JSON body");
    assert_eq!(body["instId"], "BTC-USDT-SWAP");
    assert_eq!(body["tdMode"], "cross");
    assert_eq!(body["side"], "sell");
    assert_eq!(body["ordType"], "conditional");
    assert_eq!(body["sz"], "5");
    assert_eq!(body["posSide"], "long");
    assert_eq!(body["slTriggerPx"], "94");
    assert_eq!(body["slOrdPx"], "-1");
    assert!(body.get("reduceOnly").is_none());
    assert!(body
        .get("algoClOrdId")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty()));

    let row = sqlx::query(
        r#"
        SELECT price, status, success, action, order_type, order_id, client_order_id
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-standalone-risk-order")
    .fetch_one(&pool)
    .await
    .expect("standalone risk order should be persisted");
    assert_eq!(row.get::<Option<f64>, _>("price"), None);
    assert_eq!(row.get::<String, _>("status"), "algo_submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(row.get::<String, _>("order_id"), "algo-1");
    assert!(!row.get::<String, _>("client_order_id").trim().is_empty());

    let row_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM live_order_records WHERE run_id = ?")
            .bind("run-standalone-risk-order")
            .fetch_one(&pool)
            .await
            .expect("standalone risk order row count should query");
    assert_eq!(
        row_count, 1,
        "pre-registered algo order must be updated in place instead of duplicated"
    );

    let sync_candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("ordinary order sync candidates should query");
    assert!(
        sync_candidates.is_empty(),
        "standalone algo order must not enter ordinary order sync without algo sync support"
    );
    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(algo_sync_candidates[0].order_id, "algo-1");
    assert_eq!(algo_sync_candidates[0].status, "algo_submitted");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn standalone_risk_order_unknown_response_keeps_pre_registered_syncable_record() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_algo_responses(vec![
            json!({"code": "0", "msg": "", "data": []}).to_string(),
        ])
        .await;
    let db_path = temp_db_path("live_action_standalone_risk_order_unknown");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "place_risk_order".to_string(),
        side: "sell".to_string(),
        price: 0.0,
        reason: "unit_test_standalone_stop_unknown".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protect_existing_long_unknown".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-standalone-risk-order-unknown",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::PlaceRiskOrder,
            "stop_market",
            Some("sell"),
            None,
            &[risk],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/order-algo")));

    let rows = sqlx::query(
        r#"
        SELECT status, success, action, order_type, order_id, client_order_id, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-standalone-risk-order-unknown")
    .fetch_all(&pool)
    .await
    .expect("standalone risk order records should query");
    assert_eq!(
        rows.len(),
        1,
        "unknown algo submit must update the pre-registered row instead of inserting a duplicate"
    );
    let row = &rows[0];
    assert_eq!(row.get::<String, _>("status"), "algo_submit_unknown");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(row.get::<String, _>("order_id"), "");
    assert!(!row.get::<String, _>("client_order_id").trim().is_empty());
    assert!(row.get::<String, _>("error_message").contains("待确认"));

    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(algo_sync_candidates[0].status, "algo_submit_unknown");
    assert_eq!(algo_sync_candidates[0].order_id, "");
    assert_eq!(
        algo_sync_candidates[0].client_order_id,
        row.get::<String, _>("client_order_id")
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn standalone_risk_order_okx_rejection_updates_pre_registered_row_without_duplicate() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_algo_responses(vec![
            mock_okx_algo_order_reject_response(),
        ])
        .await;
    let db_path = temp_db_path("live_action_standalone_risk_order_rejected");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "place_risk_order".to_string(),
        side: "sell".to_string(),
        price: 0.0,
        reason: "unit_test_standalone_stop_rejected".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protect_existing_long_rejected".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-standalone-risk-order-rejected",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::PlaceRiskOrder,
            "stop_market",
            Some("sell"),
            None,
            &[risk],
            None,
            None,
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("51000"));
        }
        other => panic!("rejected OKX algo submit must not be treated as submitted: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/order-algo")));

    let rows = sqlx::query(
        r#"
        SELECT status, success, action, order_type, order_id, client_order_id, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-standalone-risk-order-rejected")
    .fetch_all(&pool)
    .await
    .expect("standalone rejected risk order records should query");
    assert_eq!(
        rows.len(),
        1,
        "rejected algo submit must update the pre-registered row instead of inserting a duplicate"
    );
    let row = &rows[0];
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(row.get::<String, _>("order_id"), "");
    assert!(!row.get::<String, _>("client_order_id").trim().is_empty());
    assert!(row.get::<String, _>("error_message").contains("51000"));
    assert!(row
        .get::<String, _>("error_message")
        .contains("unit test algo order rejected"));

    let ordinary_sync_candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("ordinary order sync candidates should query");
    assert!(ordinary_sync_candidates.is_empty());
    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert!(
        algo_sync_candidates.is_empty(),
        "terminal rejected algo submit must not stay in active algo sync candidates"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn standalone_risk_order_uses_explicit_exchange_size_without_full_position_clamp() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_risk_explicit_exchange_size");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "place_risk_order".to_string(),
        side: "sell".to_string(),
        price: 100.0,
        reason: "unit_test_standalone_stop_explicit_size".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protect_existing_long_explicit_size".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-risk-explicit-exchange-size",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::PlaceRiskOrder,
            "stop_market",
            Some("sell"),
            Some("2.03"),
            &[risk],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order-algo"))
        .expect("standalone risk order should submit OKX algo order");
    let body = request_json_body(algo_request).expect("algo order should have JSON body");
    assert_eq!(body["side"], "sell");
    assert_eq!(body["sz"], "2.03");
    assert_eq!(body["posSide"], "long");
    assert_eq!(body["slTriggerPx"], "94");

    let row = sqlx::query(
        r#"
        SELECT size, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-risk-explicit-exchange-size")
    .fetch_one(&pool)
    .await
    .expect("submitted risk order should be persisted");
    assert_eq!(row.get::<f64, _>("size"), 2.03);
    assert_eq!(row.get::<String, _>("status"), "algo_submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_action_cancels_local_algo_risk_order_with_okx_cancel_algos() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_algo_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
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
        "algo submitted",
        "run-cancel-algo-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-1",
        "clalgo1",
    )
    .await
    .expect("local algo order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_cancel_algo_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "algo-1".to_string(),
        client_order_id: "clalgo1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-algo-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-algos")));
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")));
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("algo-1")
    .fetch_one(&pool)
    .await
    .expect("algo order should still exist");
    assert_eq!(row.get::<String, _>("status"), "algo_cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("保护单撤销请求已提交"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_action_cancels_local_algo_risk_order_by_client_order_id() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_algo_order_by_client_id");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "algo submitting",
        "run-cancel-algo-order-by-client-id",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "clalgo1",
    )
    .await
    .expect("local pre-registered algo order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_cancel_algo_order_by_client_id".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "clalgo1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-algo-order-by-client-id",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let cancel_algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-algos"))
        .expect("algo risk order cancel should submit OKX cancel-algos");
    let body = request_json_body(cancel_algo_request).expect("cancel-algos should have JSON body");
    let item = body
        .as_array()
        .and_then(|items| items.first())
        .expect("cancel-algos body should contain one item");
    assert_eq!(item["instId"], "BTC-USDT-SWAP");
    assert!(item.get("algoId").is_none());
    assert_eq!(item["algoClOrdId"], "clalgo1");
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")));

    let row = sqlx::query(
        r#"
        SELECT order_id, client_order_id, status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("clalgo1")
    .fetch_one(&pool)
    .await
    .expect("algo order should still exist");
    assert_eq!(row.get::<String, _>("order_id"), "algo-1");
    assert_eq!(row.get::<String, _>("client_order_id"), "clalgo1");
    assert_eq!(row.get::<String, _>("status"), "algo_cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("保护单撤销请求已提交"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn explicit_cancel_algo_symbol_scopes_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_algo_symbol_collision");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let btc_config = test_config();
    let mut eth_config = btc_config.clone();
    eth_config.symbol = "ETH-USDT-SWAP".to_string();

    insert_live_exchange_order(
        &pool,
        &eth_config,
        "sell",
        "stop_market",
        1.0,
        194.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "eth algo submitting",
        "run-cancel-algo-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedalgoclient",
    )
    .await
    .expect("eth algo order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "btc algo submitting",
        "run-cancel-algo-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedalgoclient",
    )
    .await
    .expect("btc algo order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_algo_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedalgoclient".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-algo-collision",
            &eth_config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let cancel_algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-algos"))
        .expect("algo risk order cancel should submit OKX cancel-algos");
    let body = request_json_body(cancel_algo_request).expect("cancel-algos should have JSON body");
    let item = body
        .as_array()
        .and_then(|items| items.first())
        .expect("cancel-algos body should contain one item");
    assert_eq!(item["instId"], "ETH-USDT-SWAP");
    assert_eq!(item["algoClOrdId"], "sharedalgoclient");

    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedalgoclient")
    .fetch_all(&pool)
    .await
    .expect("local algo orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "algo_submitting");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "algo_cancel_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_algo_without_action_symbol_rejects_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_algo_unscoped_symbol_collision");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let btc_config = test_config();
    let mut eth_config = btc_config.clone();
    eth_config.symbol = "ETH-USDT-SWAP".to_string();

    insert_live_exchange_order(
        &pool,
        &eth_config,
        "sell",
        "stop_market",
        1.0,
        194.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "eth algo submitting",
        "run-cancel-algo-unscoped-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedunscopedalgo",
    )
    .await
    .expect("eth algo order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "btc algo submitting",
        "run-cancel-algo-unscoped-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedunscopedalgo",
    )
    .await
    .expect("btc algo order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_algo_unscoped_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedunscopedalgo".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Algo,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-algo-unscoped-collision",
            &eth_config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("多个交易对"));
            assert!(reason.contains("显式提供 symbol"));
        }
        other => panic!("unscoped algo identity collision should not submit cancel: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/cancel-algos")),
        "ambiguous unscoped algo cancel must not reach OKX cancel-algos"
    );
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedunscopedalgo")
    .fetch_all(&pool)
    .await
    .expect("local algo orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "algo_submitting");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "algo_submitting");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_target_kind_any_rejects_exchange_algo_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_any_exchange_algo_collision");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();

    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "ordinary submitted",
        "run-cancel-any-kind-collision-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedkindcancel",
    )
    .await
    .expect("ordinary order should insert");
    insert_live_exchange_order(
        &pool,
        &config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "algo submitting",
        "run-cancel-any-kind-collision-algo",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedkindcancel",
    )
    .await
    .expect("algo order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_any_kind_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedkindcancel".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-any-kind-collision",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("target_order_kind=any"));
            assert!(reason.contains("普通订单和保护单"));
            assert!(reason.contains("target_order_kind=exchange/algo"));
        }
        other => panic!("ambiguous any target kind cancel should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded.iter().any(|request| {
            request.contains("/api/v5/trade/cancel-order")
                || request.contains("/api/v5/trade/cancel-algos")
        }),
        "ambiguous any target kind cancel must not reach OKX cancel endpoints"
    );
    let rows = sqlx::query(
        r#"
        SELECT action, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY action ASC
        "#,
    )
    .bind("sharedkindcancel")
    .fetch_all(&pool)
    .await
    .expect("local orders should query");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("action") == "open_position"
            && row.get::<String, _>("status") == "submitted"
    }));
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("action") == "place_risk_order"
            && row.get::<String, _>("status") == "algo_submitting"
    }));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_action_with_explicit_algo_target_records_external_sync_candidate() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_external_algo_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_external_algo_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgo1".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-external-algo-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let cancel_algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-algos"))
        .expect("explicit algo target should submit OKX cancel-algos");
    let body = request_json_body(cancel_algo_request).expect("cancel-algos should have JSON body");
    let item = body
        .as_array()
        .and_then(|items| items.first())
        .expect("cancel-algos body should contain one item");
    assert_eq!(item["instId"], "BTC-USDT-SWAP");
    assert!(item.get("algoId").is_none());
    assert_eq!(item["algoClOrdId"], "externalalgo1");
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, size
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalalgo1")
    .fetch_one(&pool)
    .await
    .expect("external algo cancel sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "conditional");
    assert_eq!(row.get::<String, _>("order_id"), "algo-1");
    assert_eq!(row.get::<String, _>("status"), "algo_cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert_eq!(row.get::<f64, _>("size"), 0.0);

    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(algo_sync_candidates[0].client_order_id, "externalalgo1");
    assert_eq!(algo_sync_candidates[0].status, "algo_cancel_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_external_algo_unknown_response_records_sync_candidate() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_algo_responses(vec![
            json!({"code": "0", "msg": "", "data": []}).to_string(),
        ])
        .await;
    let db_path = temp_db_path("live_action_cancel_external_algo_unknown");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_external_algo_unknown".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgocancelunk".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-external-algo-unknown",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::CancelOrder,
            "market",
            None,
            None,
            &[],
            None,
            Some(&cancel),
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-algos")));
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalalgocancelunk")
    .fetch_one(&pool)
    .await
    .expect("external unknown algo cancel sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "conditional");
    assert_eq!(row.get::<String, _>("order_id"), "");
    assert_eq!(row.get::<String, _>("status"), "algo_cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("待确认"));

    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(
        algo_sync_candidates[0].client_order_id,
        "externalalgocancelunk"
    );
    assert_eq!(algo_sync_candidates[0].status, "algo_cancel_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_action_amends_local_algo_risk_order_with_okx_amend_algos() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_algo_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
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
        "run-modify-algo-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-1",
        "clalgo1",
    )
    .await
    .expect("local algo order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_algo_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "algo-1".to_string(),
        client_order_id: "clalgo1".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "algoamend1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-algo-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let amend_algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/amend-algos"))
        .expect("algo risk order modify should submit OKX amend-algos");
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-order")));
    let body = request_json_body(amend_algo_request).expect("amend-algos should have JSON body");
    assert_eq!(body["instId"], "BTC-USDT-SWAP");
    assert_eq!(body["algoId"], "algo-1");
    assert_eq!(body["algoClOrdId"], "clalgo1");
    assert_eq!(body["newSz"], "2");
    assert_eq!(body["newSlTriggerPx"], "94.5");
    assert_eq!(body["newSlOrdPx"], "-1");
    assert_eq!(body["newSlTriggerPxType"], "last");
    assert_eq!(body["cxlOnFail"], true);
    assert_eq!(body["reqId"], "algoamend1");
    assert!(body.get("newTpTriggerPx").is_none());
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("algo-1")
    .fetch_one(&pool)
    .await
    .expect("algo order should still exist");
    assert_eq!(row.get::<String, _>("status"), "algo_modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("保护单改单请求已提交"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_algo_without_action_symbol_rejects_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_algo_unscoped_symbol_collision");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let btc_config = test_config();
    let mut eth_config = btc_config.clone();
    eth_config.symbol = "ETH-USDT-SWAP".to_string();

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
        "run-modify-algo-unscoped-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedunscopedalgomodify",
    )
    .await
    .expect("eth algo order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_live",
        true,
        "btc algo live",
        "run-modify-algo-unscoped-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedunscopedalgomodify",
    )
    .await
    .expect("btc algo order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_algo_unscoped_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedunscopedalgomodify".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "algounscopedamend1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Algo,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-algo-unscoped-collision",
            &eth_config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("多个交易对"));
            assert!(reason.contains("显式提供 symbol"));
        }
        other => panic!("unscoped algo identity collision should not submit modify: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-algos")),
        "ambiguous unscoped algo modify must not reach OKX amend-algos"
    );
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedunscopedalgomodify")
    .fetch_all(&pool)
    .await
    .expect("local algo orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "algo_live");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "algo_live");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_target_kind_any_rejects_exchange_algo_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_any_exchange_algo_collision");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();

    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "ordinary submitted",
        "run-modify-any-kind-collision-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedkindmodify",
    )
    .await
    .expect("ordinary order should insert");
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
        "run-modify-any-kind-collision-algo",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedkindmodify",
    )
    .await
    .expect("algo order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_any_kind_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedkindmodify".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "modifyanykindcollision1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-any-kind-collision",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("target_order_kind=any"));
            assert!(reason.contains("普通订单和保护单"));
            assert!(reason.contains("target_order_kind=exchange/algo"));
        }
        other => panic!("ambiguous any target kind modify should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded.iter().any(|request| {
            request.contains("/api/v5/trade/amend-order")
                || request.contains("/api/v5/trade/amend-algos")
        }),
        "ambiguous any target kind modify must not reach OKX amend endpoints"
    );
    let rows = sqlx::query(
        r#"
        SELECT action, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY action ASC
        "#,
    )
    .bind("sharedkindmodify")
    .fetch_all(&pool)
    .await
    .expect("local orders should query");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("action") == "open_position"
            && row.get::<String, _>("status") == "submitted"
    }));
    assert!(rows.iter().any(|row| {
        row.get::<String, _>("action") == "place_risk_order"
            && row.get::<String, _>("status") == "algo_live"
    }));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_action_with_explicit_algo_target_records_external_sync_candidate() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_external_algo_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_external_algo_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgo2".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "externalalgoamend1".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
        target_order_type: Some("stop_market".to_string()),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-external-algo-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let amend_algo_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/amend-algos"))
        .expect("explicit algo target should submit OKX amend-algos");
    let body = request_json_body(amend_algo_request).expect("amend-algos should have JSON body");
    assert_eq!(body["instId"], "BTC-USDT-SWAP");
    assert!(body.get("algoId").is_none());
    assert_eq!(body["algoClOrdId"], "externalalgo2");
    assert_eq!(body["newSz"], "2");
    assert_eq!(body["newSlTriggerPx"], "94.5");
    assert_eq!(body["reqId"], "externalalgoamend1");
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-order")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, size
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalalgo2")
    .fetch_one(&pool)
    .await
    .expect("external algo modify sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(row.get::<String, _>("order_id"), "algo-1");
    assert_eq!(row.get::<String, _>("status"), "algo_modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert_eq!(row.get::<f64, _>("size"), 0.0);

    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(algo_sync_candidates[0].client_order_id, "externalalgo2");
    assert_eq!(algo_sync_candidates[0].status, "algo_modify_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_external_algo_requires_target_order_type_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_external_algo_without_type");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_external_algo_without_type".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgo-missing-type".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "externalalgomissingtype1".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-external-algo-without-type",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("target_order_type"));
        }
        other => {
            panic!("external algo modify without target_order_type should not submit: {other:?}")
        }
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-algos")),
        "external algo modify without target_order_type must be rejected before OKX amend-algos"
    );
    let row_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM live_order_records WHERE run_id = ?")
            .bind("run-modify-external-algo-without-type")
            .fetch_one(&pool)
            .await
            .expect("local order row count should query");
    assert_eq!(row_count, 0);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_external_algo_rejects_unsupported_target_order_type_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_external_algo_bad_type");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_external_algo_bad_type".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgo-bad-type".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "externalalgobadtype1".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
        target_order_type: Some("take_profit_limit".to_string()),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-external-algo-bad-type",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("target_order_type"));
        }
        other => {
            panic!("unsupported external algo target_order_type should not submit: {other:?}")
        }
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-algos")),
        "unsupported external algo target_order_type must be rejected before OKX amend-algos"
    );
    let row_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM live_order_records WHERE run_id = ?")
            .bind("run-modify-external-algo-bad-type")
            .fetch_one(&pool)
            .await
            .expect("local order row count should query");
    assert_eq!(row_count, 0);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_external_algo_unknown_response_records_sync_candidate() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_algo_responses(vec![
            json!({"code": "0", "msg": "", "data": []}).to_string(),
        ])
        .await;
    let db_path = temp_db_path("live_action_modify_external_algo_unknown");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
    let config = test_config();
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_external_algo_unknown".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "externalalgounknown".to_string(),
        new_size: Some("2.00".to_string()),
        new_price: Some("94.50".to_string()),
        cancel_on_fail: true,
        request_id: "externalalgounknown1".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Algo,
        target_order_type: Some("stop-market".to_string()),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-external-algo-unknown",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-algos")));
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-order")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalalgounknown")
    .fetch_one(&pool)
    .await
    .expect("external unknown algo modify sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "place_risk_order");
    assert_eq!(row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(row.get::<String, _>("order_id"), "");
    assert_eq!(row.get::<String, _>("status"), "algo_modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("待确认"));

    let algo_sync_candidates =
        crate::live_strategy::storage::query_live_algo_order_sync_candidates(
            &pool,
            &config.mode,
            &config.strategy_id,
            10,
        )
        .await
        .expect("algo order sync candidates should query");
    assert_eq!(algo_sync_candidates.len(), 1);
    assert_eq!(
        algo_sync_candidates[0].client_order_id,
        "externalalgounknown"
    );
    assert_eq!(algo_sync_candidates[0].status, "algo_modify_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_algo_new_price_must_match_okx_tick_size_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_algo_tick_size");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let public_client =
        OkxPublicClient::new_with_proxy(base_url.clone(), "direct").expect("public client");
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
    let runtime = LiveStrategyRuntime::new();
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
        "run-modify-algo-tick",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-1",
        "clalgo1",
    )
    .await
    .expect("local algo order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_modify_algo_tick".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_100,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "algo-1".to_string(),
        client_order_id: "clalgo1".to_string(),
        new_size: None,
        new_price: Some("94.505".to_string()),
        cancel_on_fail: true,
        request_id: "algoamendbad1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-algo-tick",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ModifyOrder,
            "market",
            None,
            None,
            &[],
            None,
            None,
            Some(&modify),
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("tickSz"));
        }
        other => panic!("invalid algo amend trigger price should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-algos")),
        "invalid algo amend trigger price must be rejected before OKX submit"
    );
    let row = sqlx::query("SELECT status FROM live_order_records WHERE order_id = ?")
        .bind("algo-1")
        .fetch_one(&pool)
        .await
        .expect("algo order should still exist");
    assert_eq!(row.get::<String, _>("status"), "algo_live");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
#[test]
fn risk_order_maps_to_okx_attached_stop_loss() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protective_stop".to_string(),
    };

    let algos = attached_algo_orders_from_risk_orders(&[risk], "BTC-USDT-SWAP", "buy", 100.0, None)
        .expect("stop risk order should map to attached algo");

    assert_eq!(algos.len(), 1);
    assert_eq!(algos[0].sl_trigger_px.as_deref(), Some("94"));
    assert_eq!(algos[0].sl_ord_px.as_deref(), Some("-1"));
    assert_eq!(algos[0].sl_trigger_px_type.as_deref(), Some("last"));
}

#[test]
fn ambiguous_ratio_risk_order_without_type_is_rejected() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "".to_string(),
        trigger_price: None,
        stop_loss: Some(0.05),
        take_profit: Some(0.10),
        reason: "ambiguous_ratio_risk".to_string(),
    };

    let error = attached_algo_orders_from_risk_orders(&[risk], "BTC-USDT-SWAP", "buy", 100.0, None)
        .expect_err("ambiguous ratio-only risk order should be rejected");

    assert!(error.to_string().contains("暂不支持"));
}

#[test]
fn ratio_risk_order_without_explicit_trigger_must_match_tick_size() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: None,
        stop_loss: Some(0.045),
        take_profit: None,
        reason: "ratio_stop_off_tick".to_string(),
    };
    let rules = InstrumentOrderRules {
        inst_id: "BTC-USDT-SWAP".to_string(),
        state: "live".to_string(),
        min_sz: Some(0.01),
        lot_sz: Some(0.01),
        tick_sz: Some(0.0001),
        ct_val: Some(0.01),
        ct_val_ccy: "BTC".to_string(),
    };

    let error = attached_algo_orders_from_risk_orders(
        &[risk],
        "BTC-USDT-SWAP",
        "buy",
        0.8854,
        Some(&rules),
    )
    .expect_err("derived ratio stop trigger should not be silently aligned");

    assert!(error.to_string().contains("tickSz"));
}

#[test]
fn spot_standalone_ratio_risk_without_trigger_is_rejected_without_average_price() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT".to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.05),
        take_profit: None,
        reason: "spot_ratio_stop_without_average".to_string(),
    };
    let close_order = ResolvedRiskCloseOrder {
        order_side: "sell".to_string(),
        quantity: 1.0,
        average_price: None,
    };
    let kind = attached_risk_kind(&risk).expect("risk kind should resolve");

    let error = standalone_risk_trigger_price(kind, &close_order, &risk)
        .expect_err("spot standalone ratio risk needs explicit trigger");

    assert!(error.to_string().contains("缺少有效触发价"));
}

#[test]
fn risk_order_with_wrong_close_side_is_rejected() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "buy".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "wrong_side".to_string(),
    };

    let error = attached_algo_orders_from_risk_orders(&[risk], "BTC-USDT-SWAP", "buy", 100.0, None)
        .expect_err("same-side protective order should be rejected");

    assert!(error.to_string().contains("保护单方向"));
}

#[test]
fn long_stop_loss_trigger_above_entry_is_rejected() {
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(106.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "stop_on_profitable_side".to_string(),
    };

    let error = attached_algo_orders_from_risk_orders(&[risk], "BTC-USDT-SWAP", "buy", 100.0, None)
        .expect_err("long stop loss above entry should be rejected");

    assert!(error.to_string().contains("触发价方向"));
}
