use super::*;

#[tokio::test]
async fn cancel_order_action_submits_okx_cancel_and_marks_order_requested() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_order");
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
        "submitted",
        "run-cancel-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "cancel-target-order",
        "canceltargetclient",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_cancel_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "cancel-target-order".to_string(),
        client_order_id: "canceltargetclient".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-order",
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
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("cancel-target-order")
    .fetch_one(&pool)
    .await
    .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("撤单请求已提交"));
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")
            && request.contains("cancel-target-order")
            && request.contains("canceltargetclient")));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
#[tokio::test]
async fn cancel_order_unknown_response_marks_order_requested_for_sync() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server_with_trade_responses(
        Vec::new(),
        vec!["not-json".to_string()],
        Vec::new(),
        Vec::new(),
    )
    .await;
    let db_path = temp_db_path("live_action_cancel_order_unknown");
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
        "submitted",
        "run-cancel-order-unknown",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "cancel-unknown-order",
        "cancelunknowncl",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_cancel_unknown".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "cancel-unknown-order".to_string(),
        client_order_id: "cancelunknowncl".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-order-unknown",
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
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("cancel-unknown-order")
    .fetch_one(&pool)
    .await
    .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row
        .get::<String, _>("error_message")
        .contains("响应结果待确认"));
    let candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("sync candidates should query");
    assert!(candidates.iter().any(|candidate| {
        candidate.order_id == "cancel-unknown-order" && candidate.status == "cancel_requested"
    }));
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-order")));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_without_action_symbol_uses_local_order_symbol() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_order_local_symbol");
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
    let mut local_order_config = config.clone();
    local_order_config.symbol = "ETH-USDT-SWAP".to_string();
    insert_live_exchange_order(
        &pool,
        &local_order_config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "submitted",
        "run-cancel-local-symbol",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "cancel-eth-order",
        "cancelethclient",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_local_symbol".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "cancel-eth-order".to_string(),
        client_order_id: "cancelethclient".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-local-symbol",
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
    let cancel_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-order"))
        .expect("cancel request should be submitted");
    let body = request_json_body(cancel_request).expect("cancel order should have JSON body");
    assert_eq!(body["instId"].as_str(), Some("ETH-USDT-SWAP"));
    assert_eq!(body["ordId"].as_str(), Some("cancel-eth-order"));
    assert_eq!(body["clOrdId"].as_str(), Some("cancelethclient"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn explicit_cancel_order_symbol_scopes_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_order_symbol_collision");
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
        "buy",
        "limit",
        1.0,
        200.0,
        "open_position",
        "submitted",
        true,
        "eth submitted",
        "run-cancel-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedcollisionclient",
    )
    .await
    .expect("eth local order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "btc submitted",
        "run-cancel-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedcollisionclient",
    )
    .await
    .expect("btc local order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedcollisionclient".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Exchange,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-collision",
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
    let cancel_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-order"))
        .expect("cancel request should be submitted");
    let body = request_json_body(cancel_request).expect("cancel order should have JSON body");
    assert_eq!(body["instId"].as_str(), Some("ETH-USDT-SWAP"));
    assert_eq!(body["clOrdId"].as_str(), Some("sharedcollisionclient"));

    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedcollisionclient")
    .fetch_all(&pool)
    .await
    .expect("local orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "submitted");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "cancel_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_without_action_symbol_rejects_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_order_unscoped_symbol_collision");
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
        "buy",
        "limit",
        1.0,
        200.0,
        "open_position",
        "submitted",
        true,
        "eth submitted",
        "run-cancel-unscoped-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedunscopedcancel",
    )
    .await
    .expect("eth local order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "btc submitted",
        "run-cancel-unscoped-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedunscopedcancel",
    )
    .await
    .expect("btc local order should insert");
    let action_record = StrategyActionRecord {
        action: "cancel_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_cancel_unscoped_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedunscopedcancel".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Exchange,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-unscoped-collision",
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
        other => panic!("unscoped identity collision should not submit cancel: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/cancel-order")),
        "ambiguous unscoped cancel must not reach OKX cancel-order"
    );
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedunscopedcancel")
    .fetch_all(&pool)
    .await
    .expect("local orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "submitted");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "submitted");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_without_local_order_requires_explicit_action_symbol_scope() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_order_missing_local_unscoped");
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
        reason: "unit_test_cancel_missing_local_unscoped".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "missing-local-order".to_string(),
        client_order_id: "missinglocalclient".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-missing-local-unscoped",
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
            assert!(reason.contains("未显式提供 symbol"));
        }
        other => panic!("unscoped missing local cancel should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/cancel-order")),
        "unscoped missing local cancel must not reach OKX"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn cancel_order_with_explicit_exchange_target_records_external_sync_candidate() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_cancel_external_exchange_order");
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
        reason: "unit_test_cancel_external_exchange_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let cancel = StrategyCancelOrderIntent {
        order_id: "external-exchange-order".to_string(),
        client_order_id: "externalexchangeclient".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Exchange,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-cancel-external-exchange-order",
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
    let cancel_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/cancel-order"))
        .expect("explicit exchange target should submit OKX cancel-order");
    let body = request_json_body(cancel_request).expect("cancel order should have JSON body");
    assert_eq!(body["instId"].as_str(), Some("BTC-USDT-SWAP"));
    assert_eq!(body["ordId"].as_str(), Some("external-exchange-order"));
    assert_eq!(body["clOrdId"].as_str(), Some("externalexchangeclient"));
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/cancel-algos")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, size
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalexchangeclient")
    .fetch_one(&pool)
    .await
    .expect("external exchange cancel sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "cancel_order");
    assert_eq!(row.get::<String, _>("order_type"), "market");
    assert_eq!(row.get::<String, _>("order_id"), "external-exchange-order");
    assert_eq!(row.get::<String, _>("status"), "cancel_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert_eq!(row.get::<f64, _>("size"), 0.0);

    let sync_candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("ordinary order sync candidates should query");
    assert_eq!(sync_candidates.len(), 1);
    assert_eq!(sync_candidates[0].client_order_id, "externalexchangeclient");
    assert_eq!(sync_candidates[0].status, "cancel_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_action_submits_okx_amend_and_marks_order_requested() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order");
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
        "submitted",
        "run-modify-order",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "modify-target-order",
        "modifytargetclient",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "modify-target-order".to_string(),
        client_order_id: "modifytargetclient".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "modifyreq1".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-order",
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
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("modify-target-order")
    .fetch_one(&pool)
    .await
    .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("改单请求已提交"));
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded.iter().any(|request| {
        request.contains("/api/v5/trade/amend-order")
            && request.contains("modify-target-order")
            && request.contains("modifytargetclient")
            && request.contains(r#""newSz":"2""#)
            && request.contains(r#""newPx":"101.25""#)
            && request.contains(r#""cxlOnFail":true"#)
            && request.contains(r#""reqId":"modifyreq1""#)
    }));
    let logs = runtime.execution_logs("run-modify-order", 20).await;
    let submit_log = logs
        .iter()
        .find(|entry| entry.message.contains("准备提交 OKX 改单请求"))
        .expect("modify submit log should exist");
    assert_eq!(submit_log.details["target_order_kind"], json!("any"));
    assert!(submit_log.details["target_order_type"].is_null());
    assert_eq!(submit_log.details["scope_explicit"], json!(false));
    assert_eq!(submit_log.details["cancel_on_fail"], json!(true));
    assert_eq!(submit_log.details["request_id"], json!("modifyreq1"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_unknown_response_marks_order_requested_for_sync() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server_with_trade_responses(
        Vec::new(),
        Vec::new(),
        vec!["not-json".to_string()],
        Vec::new(),
    )
    .await;
    let db_path = temp_db_path("live_action_modify_order_unknown");
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
        "submitted",
        "run-modify-order-unknown",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "modify-unknown-order",
        "modifyunknowncl",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_unknown".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "modify-unknown-order".to_string(),
        client_order_id: "modifyunknowncl".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "modifyunknownreq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-order-unknown",
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
    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("modify-unknown-order")
    .fetch_one(&pool)
    .await
    .expect("order should exist");
    assert_eq!(row.get::<String, _>("status"), "modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row
        .get::<String, _>("error_message")
        .contains("响应结果待确认"));
    let candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("sync candidates should query");
    assert!(candidates.iter().any(|candidate| {
        candidate.order_id == "modify-unknown-order" && candidate.status == "modify_requested"
    }));
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-order")));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_without_action_symbol_uses_local_order_symbol() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order_local_symbol");
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
    let mut local_order_config = config.clone();
    local_order_config.symbol = "ETH-USDT-SWAP".to_string();
    insert_live_exchange_order(
        &pool,
        &local_order_config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "submitted",
        "run-modify-local-symbol",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "modify-eth-order",
        "modifyethclient",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_local_symbol".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "modify-eth-order".to_string(),
        client_order_id: "modifyethclient".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "modifyethreq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-local-symbol",
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
    let amend_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/amend-order"))
        .expect("amend request should be submitted");
    let body = request_json_body(amend_request).expect("amend order should have JSON body");
    assert_eq!(body["instId"].as_str(), Some("ETH-USDT-SWAP"));
    assert_eq!(body["ordId"].as_str(), Some("modify-eth-order"));
    assert_eq!(body["clOrdId"].as_str(), Some("modifyethclient"));
    assert_eq!(body["newSz"].as_str(), Some("2"));
    assert_eq!(body["newPx"].as_str(), Some("101.25"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_without_action_symbol_rejects_local_identity_collision() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order_unscoped_symbol_collision");
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
        "buy",
        "limit",
        1.0,
        200.0,
        "open_position",
        "submitted",
        true,
        "eth submitted",
        "run-modify-unscoped-collision-eth",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "sharedunscopedmodify",
    )
    .await
    .expect("eth local order should insert");
    insert_live_exchange_order(
        &pool,
        &btc_config,
        "buy",
        "limit",
        1.0,
        100.0,
        "open_position",
        "submitted",
        true,
        "btc submitted",
        "run-modify-unscoped-collision-btc",
        1_780_000_000_001,
        ArrivalQuote::default(),
        "",
        "sharedunscopedmodify",
    )
    .await
    .expect("btc local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_unscoped_symbol_collision".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: String::new(),
        client_order_id: "sharedunscopedmodify".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "modifyunscopedcollisionreq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Exchange,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-unscoped-collision",
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
        other => panic!("unscoped identity collision should not submit modify: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-order")),
        "ambiguous unscoped modify must not reach OKX amend-order"
    );
    let rows = sqlx::query(
        r#"
        SELECT inst_id, status
        FROM live_order_records
        WHERE client_order_id = ?
        ORDER BY inst_id ASC
        "#,
    )
    .bind("sharedunscopedmodify")
    .fetch_all(&pool)
    .await
    .expect("local orders should query");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("inst_id"), "BTC-USDT-SWAP");
    assert_eq!(rows[0].get::<String, _>("status"), "submitted");
    assert_eq!(rows[1].get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(rows[1].get::<String, _>("status"), "submitted");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_without_local_order_requires_explicit_action_symbol_scope() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order_missing_local_unscoped");
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
        price: 0.0,
        reason: "unit_test_modify_missing_local_unscoped".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "missing-local-order".to_string(),
        client_order_id: "missinglocalclient".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "modifymissinglocalreq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-missing-local-unscoped",
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
            assert!(reason.contains("未显式提供 symbol"));
        }
        other => panic!("unscoped missing local modify should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-order")),
        "unscoped missing local modify must not reach OKX"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_with_explicit_exchange_target_records_external_sync_candidate() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_external_exchange_order");
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
        price: 0.0,
        reason: "unit_test_modify_external_exchange_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "external-modify-order".to_string(),
        client_order_id: "externalmodifyclient".to_string(),
        new_size: Some("2".to_string()),
        new_price: Some("101.25".to_string()),
        cancel_on_fail: true,
        request_id: "externalmodifyreq".to_string(),
        scope_explicit: true,
        target_kind: StrategyOrderTargetKind::Exchange,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-external-exchange-order",
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
    let amend_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/amend-order"))
        .expect("explicit exchange target should submit OKX amend-order");
    let body = request_json_body(amend_request).expect("amend order should have JSON body");
    assert_eq!(body["instId"].as_str(), Some("BTC-USDT-SWAP"));
    assert_eq!(body["ordId"].as_str(), Some("external-modify-order"));
    assert_eq!(body["clOrdId"].as_str(), Some("externalmodifyclient"));
    assert_eq!(body["newSz"].as_str(), Some("2"));
    assert_eq!(body["newPx"].as_str(), Some("101.25"));
    assert!(!recorded
        .iter()
        .any(|request| request.contains("/api/v5/trade/amend-algos")));

    let row = sqlx::query(
        r#"
        SELECT action, order_type, order_id, client_order_id, status, success, size
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("externalmodifyclient")
    .fetch_one(&pool)
    .await
    .expect("external exchange modify sync candidate should be recorded");
    assert_eq!(row.get::<String, _>("action"), "modify_order");
    assert_eq!(row.get::<String, _>("order_type"), "market");
    assert_eq!(row.get::<String, _>("order_id"), "external-modify-order");
    assert_eq!(row.get::<String, _>("status"), "modify_requested");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert_eq!(row.get::<f64, _>("size"), 0.0);

    let sync_candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("ordinary order sync candidates should query");
    assert_eq!(sync_candidates.len(), 1);
    assert_eq!(sync_candidates[0].client_order_id, "externalmodifyclient");
    assert_eq!(sync_candidates[0].status, "modify_requested");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_new_price_must_match_okx_tick_size_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order_tick_size");
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
        "submitted",
        "run-modify-price-tick",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "modify-price-target",
        "modifypricetarget",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_price_tick".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "modify-price-target".to_string(),
        client_order_id: "modifypricetarget".to_string(),
        new_size: None,
        new_price: Some("101.255".to_string()),
        cancel_on_fail: true,
        request_id: "modifypricereq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-price-tick",
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
        other => panic!("invalid amend price should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-order")),
        "invalid amend price must be rejected before OKX amend submit"
    );
    let row = sqlx::query("SELECT status FROM live_order_records WHERE order_id = ?")
        .bind("modify-price-target")
        .fetch_one(&pool)
        .await
        .expect("target order should still exist");
    assert_eq!(row.get::<String, _>("status"), "submitted");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn modify_order_new_size_must_match_okx_lot_size_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_modify_order_lot_size");
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
        "submitted",
        "run-modify-size-lot",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "modify-size-target",
        "modifysizetarget",
    )
    .await
    .expect("local order should insert");
    let action_record = StrategyActionRecord {
        action: "modify_order".to_string(),
        side: "hold".to_string(),
        price: 0.0,
        reason: "unit_test_modify_size_lot".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let modify = StrategyModifyOrderIntent {
        order_id: "modify-size-target".to_string(),
        client_order_id: "modifysizetarget".to_string(),
        new_size: Some("2.005".to_string()),
        new_price: None,
        cancel_on_fail: true,
        request_id: "modifysizereq".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-modify-size-lot",
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
            assert!(reason.contains("lotSz"));
        }
        other => panic!("invalid amend size should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/amend-order")),
        "invalid amend size must be rejected before OKX amend submit"
    );
    let row = sqlx::query("SELECT status FROM live_order_records WHERE order_id = ?")
        .bind("modify-size-target")
        .fetch_one(&pool)
        .await
        .expect("target order should still exist");
    assert_eq!(row.get::<String, _>("status"), "submitted");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn live_action_blocks_same_direction_exposure_from_okx_positions() {
    let (base_url, stop_server) = start_mock_okx_server().await;
    let db_path = temp_db_path("live_action_same_direction_exposure");
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
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_same_direction_limit".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    runtime
        .evaluate_live_action(
            "run-risk-state",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "market",
            None,
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-risk-state")
    .fetch_one(&pool)
    .await
    .expect("risk-blocked order should be persisted");

    assert_eq!(row.get::<String, _>("status"), "risk_blocked");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row
        .get::<String, _>("error_message")
        .contains("同向相关敞口比例 70.00%"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
