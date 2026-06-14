use super::*;

#[test]
fn td_mode_from_config_validates_runtime_execution_mode() {
    let contract = test_config();
    assert_eq!(td_mode_from_config(&contract).unwrap(), "cross");

    let mut isolated = contract.clone();
    isolated.params = json!({"contract_mode": true, "td_mode": "isolated", "leverage": 3});
    assert_eq!(td_mode_from_config(&isolated).unwrap(), "isolated");

    let mut invalid = contract.clone();
    invalid.params = json!({"contract_mode": true, "td_mode": "portfolio", "leverage": 3});
    assert!(td_mode_from_config(&invalid)
        .expect_err("unsupported td_mode should be rejected")
        .to_string()
        .contains("不受支持"));

    let mut cash_contract = contract.clone();
    cash_contract.params = json!({"contract_mode": true, "td_mode": "cash", "leverage": 3});
    assert!(td_mode_from_config(&cash_contract)
        .expect_err("contract cash td_mode should be rejected")
        .to_string()
        .contains("不能用于"));

    let mut contract_mode_false = contract.clone();
    contract_mode_false.params = json!({"contract_mode": false, "leverage": 3});
    assert!(td_mode_from_config(&contract_mode_false)
        .expect_err("contract_mode=false on contract inst_type should be rejected")
        .to_string()
        .contains("contract_mode=false"));

    let mut spot = contract.clone();
    spot.inst_type = "SPOT".to_string();
    spot.params = json!({});
    assert_eq!(td_mode_from_config(&spot).unwrap(), "cash");

    spot.params = json!({"contract_mode": true});
    assert!(td_mode_from_config(&spot)
        .expect_err("contract_mode=true on spot inst_type should be rejected")
        .to_string()
        .contains("contract_mode=true"));

    spot.params = json!({"contract_mode": false});
    assert_eq!(td_mode_from_config(&spot).unwrap(), "cash");

    spot.params = json!({"td_mode": "isolated"});
    assert!(td_mode_from_config(&spot)
        .expect_err("spot margin td_mode should be rejected until supported")
        .to_string()
        .contains("仅支持现货 cash"));

    spot.params = json!({"td_mode": 1});
    assert!(td_mode_from_config(&spot)
        .expect_err("td_mode must be string")
        .to_string()
        .contains("必须是字符串"));
}

#[test]
fn swap_entry_quantity_converts_base_quantity_to_contract_count() {
    let config = test_config();
    let rules = InstrumentOrderRules {
        inst_id: "BTC-USDT-SWAP".to_string(),
        state: "live".to_string(),
        min_sz: Some(0.01),
        lot_sz: Some(0.01),
        tick_sz: Some(0.01),
        ct_val: Some(0.01),
        ct_val_ccy: "BTC".to_string(),
    };

    let quantity = resolve_entry_exchange_quantity_from_base(&config, &rules, 0.12345, 100.0)
        .expect("contract size should resolve");

    assert_eq!(quantity.exchange_quantity, 12.34);
    assert_eq!(quantity.base_quantity, 0.1234);
}

#[test]
fn swap_entry_quantity_rejects_size_below_minimum_after_lot_rounding() {
    let config = test_config();
    let rules = InstrumentOrderRules {
        inst_id: "BTC-USDT-SWAP".to_string(),
        state: "live".to_string(),
        min_sz: Some(1.0),
        lot_sz: Some(1.0),
        tick_sz: Some(0.01),
        ct_val: Some(0.01),
        ct_val_ccy: "BTC".to_string(),
    };

    let error = resolve_entry_exchange_quantity_from_base(&config, &rules, 0.001, 100.0)
        .expect_err("too-small contract size should be rejected");

    assert!(error.to_string().contains("小于最小下单数量"));
}

#[test]
fn close_quantity_uses_matching_okx_available_position() {
    let positions = vec![
        json!({
            "instId": "BTC-USDT-SWAP",
            "posSide": "long",
            "pos": "5",
            "availPos": "3"
        }),
        json!({
            "instId": "BTC-USDT-SWAP",
            "posSide": "short",
            "pos": "2",
            "availPos": "2"
        }),
    ];

    assert_eq!(
        close_quantity_from_positions(&positions, "BTC-USDT-SWAP", "sell")
            .expect("long available close quantity should parse"),
        Some(3.0)
    );
    assert_eq!(
        close_quantity_from_positions(&positions, "BTC-USDT-SWAP", "buy")
            .expect("short available close quantity should parse"),
        Some(2.0)
    );
}

#[test]
fn close_order_side_is_inferred_from_single_exchange_position() {
    let long_positions = vec![json!({
        "instId": "BTC-USDT-SWAP",
        "posSide": "long",
        "pos": "5",
        "availPos": "3"
    })];
    let long_close = infer_close_order_from_positions(&long_positions, "BTC-USDT-SWAP")
        .expect("single long position should infer sell close");
    assert_eq!(long_close.order_side, "sell");
    assert_eq!(long_close.quantity, 3.0);

    let short_positions = vec![json!({
        "instId": "BTC-USDT-SWAP",
        "posSide": "short",
        "pos": "2",
        "availPos": "1.5"
    })];
    let short_close = infer_close_order_from_positions(&short_positions, "BTC-USDT-SWAP")
        .expect("single short position should infer buy close");
    assert_eq!(short_close.order_side, "buy");
    assert_eq!(short_close.quantity, 1.5);
}

#[test]
fn ambiguous_dual_side_position_requires_explicit_close_side() {
    let positions = vec![
        json!({
            "instId": "BTC-USDT-SWAP",
            "posSide": "long",
            "pos": "5",
            "availPos": "3"
        }),
        json!({
            "instId": "BTC-USDT-SWAP",
            "posSide": "short",
            "pos": "2",
            "availPos": "2"
        }),
    ];

    let error = infer_close_order_from_positions(&positions, "BTC-USDT-SWAP")
        .expect_err("dual-side positions require explicit side");

    assert!(error.to_string().contains("必须显式提供"));
}

#[test]
fn close_quantity_requires_valid_okx_available_position() {
    let positions = vec![json!({
        "instId": "BTC-USDT-SWAP",
        "posSide": "long",
        "pos": "5"
    })];

    let error = close_quantity_from_positions(&positions, "BTC-USDT-SWAP", "sell")
        .expect_err("active position without availPos should be incomplete evidence");

    assert!(error.to_string().contains("availPos"));
}

#[test]
fn close_quantity_rejects_active_okx_position_without_valid_pos() {
    let positions = vec![json!({
        "instId": "BTC-USDT-SWAP",
        "posSide": "long",
        "pos": "dirty",
        "availPos": "3"
    })];

    let error = close_quantity_from_positions(&positions, "BTC-USDT-SWAP", "sell")
        .expect_err("invalid position size should be incomplete evidence");

    assert!(error.to_string().contains("有效 pos"));
}
#[tokio::test]
async fn long_short_mode_close_uses_pos_side_without_reduce_only() {
    let (base_url, stop_server) = start_mock_okx_server().await;
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
    let mut config = test_config();
    config.params = json!({});

    let context = resolve_exchange_order_context(&private_client, &config, "sell", true)
        .await
        .expect("order context should resolve");

    assert_eq!(context.pos_side, "long");
    assert!(!context.reduce_only);
    let _ = stop_server.send(());
}
#[tokio::test]
async fn spot_close_rejects_when_okx_available_sell_size_is_missing() {
    let (base_url, stop_server) = start_mock_okx_server().await;
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
    let mut config = test_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    config.params = json!({});

    let error = resolve_spot_close_quantity(&private_client, &config, "sell")
        .await
        .expect_err("spot close must not use cash balance as available sell size");

    assert!(error.to_string().contains("可卖出的 BTC"));
    let _ = stop_server.send(());
}

#[tokio::test]
async fn close_without_synced_position_is_retryable_without_submitting_order() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_close_no_position_retry");
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
    let mut config = test_config();
    config.symbol = "DOGE-USDT-SWAP".to_string();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "close_position".to_string(),
        side: "flat".to_string(),
        price: 100.0,
        reason: "unit_test_close_no_position_retry".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-close-no-position-retry",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ClosePosition,
            "market",
            Some("sell"),
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(
                retryable,
                "no-position close should release the action key for retry"
            );
            assert!(reason.contains("当前没有"));
        }
        other => panic!("expected retryable close failure, got {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "close_position without a synced closeable position must not submit an order"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[test]
fn explicit_configured_leverage_uses_contract_inst_type_and_shared_param_parsing() {
    let mut config = test_config();
    config.params = json!({"contract_mode": true, "leverage": "7"});
    assert_eq!(explicit_configured_leverage(&config).unwrap(), Some(7.0));

    config.inst_type = "FUTURES".to_string();
    config.params = json!({"leverage": "7"});
    assert_eq!(explicit_configured_leverage(&config).unwrap(), Some(7.0));

    config.inst_type = "SPOT".to_string();
    assert_eq!(explicit_configured_leverage(&config).unwrap(), None);

    config.inst_type = "SWAP".to_string();
    config.params = json!({"contract_mode": false, "leverage": 7});
    assert!(explicit_configured_leverage(&config)
        .expect_err("contract_mode=false must be rejected for live contract execution")
        .to_string()
        .contains("contract_mode=false"));

    config.params = json!({"contract_mode": "false", "leverage": 7});
    assert!(explicit_configured_leverage(&config)
        .expect_err("contract_mode='false' must be rejected for live contract execution")
        .to_string()
        .contains("contract_mode=false"));

    config.params = json!({"contract_mode": true});
    assert_eq!(explicit_configured_leverage(&config).unwrap(), None);
}

#[test]
fn explicit_configured_leverage_rejects_invalid_runtime_values() {
    let mut config = test_config();
    config.params = json!({"contract_mode": true, "leverage": "bad"});
    assert!(explicit_configured_leverage(&config)
        .expect_err("bad leverage should be rejected")
        .to_string()
        .contains("leverage"));

    config.params = json!({"contract_mode": true, "leverage": 0});
    assert!(explicit_configured_leverage(&config)
        .expect_err("zero leverage should be rejected")
        .to_string()
        .contains("有效正数"));

    config.params = json!({"contract_mode": true, "leverage": 200});
    assert!(explicit_configured_leverage(&config)
        .expect_err("oversized leverage should be rejected")
        .to_string()
        .contains("125"));
}

#[tokio::test]
async fn live_open_short_respects_contract_and_allow_short_gate_like_backtest() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_open_short_gate");
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
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "sell".to_string(),
        price: 100.0,
        reason: "unit_test_short_gate".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let mut spot_config = test_config();
    spot_config.symbol = "BTC-USDT".to_string();
    spot_config.inst_type = "SPOT".to_string();
    let spot_outcome = runtime
        .evaluate_live_action(
            "run-spot-short-gate",
            &spot_config,
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

    match spot_outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("现货实时策略暂不支持"));
        }
        other => panic!("expected terminal spot short rejection, got {other:?}"),
    }

    let mut swap_config = test_config();
    swap_config.params = json!({"allow_short": "false"});
    let swap_outcome = runtime
        .evaluate_live_action(
            "run-swap-short-disabled",
            &swap_config,
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

    match swap_outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, retryable } => {
            assert!(!retryable);
            assert!(reason.contains("策略配置不允许做空"));
        }
        other => panic!("expected terminal allow_short rejection, got {other:?}"),
    }

    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "blocked short open must not submit an OKX order: {recorded:#?}"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn hold_action_is_skipped_without_exchange_side_effects() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_hold_no_exchange_side_effect");
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
        action: "hold".to_string(),
        side: "hold".to_string(),
        price: 100.0,
        reason: "unit_test_hold".to_string(),
        strength: 0.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-hold-skip",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::Hold,
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

    assert_eq!(outcome, LiveActionExecutionOutcome::Skipped);
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded.is_empty(),
        "hold action must not submit REST requests: {recorded:#?}"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_sets_configured_leverage_before_place_order() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_leverage_before_order");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "isolated",
        "leverage": 7
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_leverage_sync".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    runtime
        .evaluate_live_action(
            "run-leverage-sync",
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

    let recorded = requests.lock().expect("recorded requests").clone();
    let leverage_index = recorded
        .iter()
        .position(|request| request.contains("/api/v5/account/set-leverage"))
        .expect("set-leverage request should be submitted");
    let order_index = recorded
        .iter()
        .position(|request| request.contains("/api/v5/trade/order"))
        .expect("order request should be submitted");
    assert!(
        leverage_index < order_index,
        "set-leverage must be submitted before the open order"
    );
    let leverage_request = &recorded[leverage_index];
    assert!(leverage_request.contains(r#""lever":"7""#));
    assert!(leverage_request.contains(r#""mgnMode":"isolated""#));
    assert!(leverage_request.contains(r#""posSide":"long""#));

    let row = sqlx::query(
        r#"
        SELECT status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-leverage-sync")
    .fetch_one(&pool)
    .await
    .expect("submitted order should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_uses_action_scoped_symbol_for_leverage_order_and_record() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_scoped_symbol_leverage");
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
    let mut config = test_config();
    config.symbol = "ETH-USDT-SWAP".to_string();
    config.inst_type = "SWAP".to_string();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "isolated",
        "leverage": 7
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_action_scoped_symbol".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-action-scoped-symbol",
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

    assert!(
        matches!(outcome, LiveActionExecutionOutcome::Submitted),
        "action-scoped symbol should submit through OKX mock, got {outcome:?}"
    );
    let recorded = requests.lock().expect("recorded requests").clone();
    let leverage_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/account/set-leverage"))
        .expect("set-leverage request should be submitted");
    let leverage_body = request_json_body(leverage_request).expect("leverage request body");
    assert_eq!(leverage_body["instId"], "ETH-USDT-SWAP");
    assert_eq!(leverage_body["mgnMode"], "isolated");
    assert_eq!(leverage_body["posSide"], "long");

    let order_request = recorded
        .iter()
        .find(|request| is_exchange_order_request(request))
        .expect("order request should be submitted");
    let order_body = request_json_body(order_request).expect("order request body");
    assert_eq!(order_body["instId"], "ETH-USDT-SWAP");
    assert_eq!(order_body["tdMode"], "isolated");
    assert_eq!(order_body["posSide"], "long");

    let row = sqlx::query(
        r#"
        SELECT inst_id, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-action-scoped-symbol")
    .fetch_one(&pool)
    .await
    .expect("submitted order should be persisted");
    assert_eq!(row.get::<String, _>("inst_id"), "ETH-USDT-SWAP");
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_rejects_invalid_configured_leverage_before_okx_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_invalid_leverage");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "isolated",
        "leverage": 200
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_invalid_leverage".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-invalid-leverage",
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

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("125"));
        }
        other => panic!("invalid leverage must not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded.is_empty(),
        "invalid leverage must be rejected before any OKX request: {recorded:#?}"
    );
    assert!(
        recorded
            .iter()
            .all(|request| !request.contains("/api/v5/account/set-leverage")),
        "invalid leverage must be rejected before set-leverage: {recorded:#?}"
    );
    assert!(
        recorded
            .iter()
            .all(|request| !request.contains("/api/v5/trade/order")),
        "invalid leverage must be rejected before order submit: {recorded:#?}"
    );

    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-invalid-leverage")
    .fetch_one(&pool)
    .await
    .expect("submit failure should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("125"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_rejects_contract_mode_false_before_okx_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_contract_mode_false");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": false,
        "td_mode": "cross",
        "leverage": 3
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_contract_mode_false".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-contract-mode-false",
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

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("contract_mode=false"));
        }
        other => panic!("contract_mode=false must not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded.is_empty(),
        "contract_mode=false must be rejected before any OKX request: {recorded:#?}"
    );

    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-contract-mode-false")
    .fetch_one(&pool)
    .await
    .expect("submit failure should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row
        .get::<String, _>("error_message")
        .contains("contract_mode=false"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_rejects_invalid_td_mode_before_okx_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_invalid_td_mode");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "portfolio",
        "leverage": 3
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_invalid_td_mode".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-invalid-td-mode",
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

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("td_mode"));
            assert!(reason.contains("不受支持"));
        }
        other => panic!("invalid td_mode must not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded.is_empty(),
        "invalid td_mode must be rejected before any OKX request: {recorded:#?}"
    );

    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-invalid-td-mode")
    .fetch_one(&pool)
    .await
    .expect("submit failure should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("td_mode"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_with_attached_risk_assigns_algo_client_id_and_persists_sync_row() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_attached_risk_client_id");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "isolated",
        "leverage": 3
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_attached_risk_client_id".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk_order = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.0),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "unit_test_attached_stop".to_string(),
    };

    runtime
        .evaluate_live_action(
            "run-attached-risk-client-id",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "market",
            None,
            None,
            &[risk_order],
            None,
            None,
            None,
            true,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("open order should be submitted");
    assert!(order_request.contains(r#""attachAlgoOrds""#));
    assert!(order_request.contains(r#""attachAlgoClOrdId":"okxq"#));
    assert!(order_request.contains(r#""slTriggerPx":"94""#));

    let rows = sqlx::query(
        r#"
        SELECT action, status, success, order_id, client_order_id, order_type, price
        FROM live_order_records
        WHERE run_id = ?
        ORDER BY action ASC, id ASC
        "#,
    )
    .bind("run-attached-risk-client-id")
    .fetch_all(&pool)
    .await
    .expect("attached risk rows should query");
    assert_eq!(rows.len(), 2);
    let risk_row = rows
        .iter()
        .find(|row| row.get::<String, _>("action") == "place_risk_order")
        .expect("attached risk order should be persisted");
    assert_eq!(risk_row.get::<String, _>("status"), "algo_submitted");
    assert_eq!(risk_row.get::<i64, _>("success"), 1);
    assert_eq!(risk_row.get::<String, _>("order_id"), "");
    assert!(risk_row
        .get::<String, _>("client_order_id")
        .starts_with("okxq"));
    assert_eq!(risk_row.get::<String, _>("order_type"), "stop_market");
    assert_eq!(risk_row.get::<f64, _>("price"), 94.0);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
#[tokio::test]
async fn swap_open_respects_explicit_limit_order_type_and_price() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_limit_order_type");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 101.25,
        reason: "unit_test_limit_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-limit-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "limit",
            None,
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("limit entry order should be submitted");
    let order_body = request_json_body(order_request).expect("order should have JSON body");
    assert_eq!(order_body["ordType"], "limit");
    assert_eq!(order_body["px"], "101.25");

    let row = sqlx::query(
        r#"
        SELECT order_type, price, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-limit-order")
    .fetch_one(&pool)
    .await
    .expect("submitted limit order should be persisted");
    assert_eq!(row.get::<String, _>("order_type"), "limit");
    assert_eq!(row.get::<f64, _>("price"), 101.25);
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_normalizes_post_only_alias_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_post_only_alias_order_type");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 101.25,
        reason: "unit_test_post_only_alias_order".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-post-only-alias-order",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "post-only",
            None,
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("post-only entry order should be submitted");
    let order_body = request_json_body(order_request).expect("order should have JSON body");
    assert_eq!(order_body["ordType"], "post_only");
    assert_eq!(order_body["px"], "101.25");

    let row = sqlx::query(
        r#"
        SELECT order_type, price, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-post-only-alias-order")
    .fetch_one(&pool)
    .await
    .expect("submitted post-only order should be persisted");
    assert_eq!(row.get::<String, _>("order_type"), "post_only");
    assert_eq!(row.get::<f64, _>("price"), 101.25);
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_open_uses_explicit_exchange_size_without_lot_rounding() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_explicit_exchange_size");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_explicit_exchange_size".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.99),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-explicit-exchange-size",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "market",
            None,
            Some("2.03"),
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("entry order should be submitted");
    let order_body = request_json_body(order_request).expect("order should have JSON body");
    assert_eq!(order_body["sz"], "2.03");

    let row = sqlx::query(
        r#"
        SELECT size, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-explicit-exchange-size")
    .fetch_one(&pool)
    .await
    .expect("submitted order should be persisted");
    assert_eq!(row.get::<f64, _>("size"), 2.03);
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn swap_close_uses_explicit_exchange_size_without_clamping_to_full_position() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_close_explicit_exchange_size");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "close_position".to_string(),
        side: "flat".to_string(),
        price: 100.0,
        reason: "unit_test_close_explicit_exchange_size".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-close-explicit-exchange-size",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ClosePosition,
            "market",
            Some("sell"),
            Some("2.03"),
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("close order should be submitted");
    let order_body = request_json_body(order_request).expect("order should have JSON body");
    assert_eq!(order_body["side"], "sell");
    assert_eq!(order_body["sz"], "2.03");
    assert_eq!(order_body["posSide"], "long");

    let row = sqlx::query(
        r#"
        SELECT size, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-close-explicit-exchange-size")
    .fetch_one(&pool)
    .await
    .expect("submitted close order should be persisted");
    assert_eq!(row.get::<f64, _>("size"), 2.03);
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn market_close_without_reference_price_submits_and_keeps_local_price_unknown() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_close_without_reference_price");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "close_position".to_string(),
        side: "flat".to_string(),
        price: 0.0,
        reason: "unit_test_market_close_without_reference_price".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-close-without-reference-price",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::ClosePosition,
            "market",
            Some("sell"),
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let recorded = requests.lock().expect("recorded requests").clone();
    let order_request = recorded
        .iter()
        .find(|request| request.contains("/api/v5/trade/order"))
        .expect("close order should be submitted");
    let order_body = request_json_body(order_request).expect("order should have JSON body");
    assert_eq!(order_body["side"], "sell");
    assert_eq!(order_body["ordType"], "market");
    assert_eq!(order_body["sz"], "5");
    assert!(order_body.get("px").is_none());

    let row = sqlx::query(
        r#"
        SELECT price, size, status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-close-without-reference-price")
    .fetch_one(&pool)
    .await
    .expect("submitted close order should be persisted");
    assert_eq!(row.get::<Option<f64>, _>("price"), None);
    assert_eq!(row.get::<f64, _>("size"), 5.0);
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<i64, _>("success"), 1);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn explicit_exchange_size_must_match_lot_size_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_explicit_exchange_size_lot");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_bad_explicit_exchange_size".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.99),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-explicit-exchange-size-lot",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "market",
            None,
            Some("2.005"),
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("lotSz"));
            assert!(reason.contains("静默改量"));
        }
        other => panic!("off-lot exchange_size should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "off-lot explicit exchange_size must be rejected before OKX order submit"
    );

    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-explicit-exchange-size-lot")
    .fetch_one(&pool)
    .await
    .expect("failed order should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("lotSz"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn limit_order_price_must_match_okx_tick_size_before_submit() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_limit_order_tick_size");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 101.255,
        reason: "unit_test_limit_tick_size".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-limit-tick-size",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "limit",
            None,
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("tickSz"));
        }
        other => panic!("invalid tick price should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "invalid tick price must be rejected before OKX order submit"
    );
    let row = sqlx::query(
        r#"
        SELECT order_type, price, status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-limit-tick-size")
    .fetch_one(&pool)
    .await
    .expect("failed limit order should be persisted");
    assert_eq!(row.get::<String, _>("order_type"), "limit");
    assert_eq!(row.get::<f64, _>("price"), 101.255);
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("tickSz"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn attached_risk_trigger_price_must_match_okx_tick_size_before_open() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_attached_risk_tick_size");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_attached_risk_tick".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let risk = StrategyRiskOrderIntent {
        symbol: "BTC-USDT-SWAP".to_string(),
        side: "sell".to_string(),
        order_type: "stop_market".to_string(),
        trigger_price: Some(94.005),
        stop_loss: Some(0.06),
        take_profit: None,
        reason: "protective_stop_off_tick".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-attached-risk-tick-size",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "market",
            None,
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
            assert!(reason.contains("保护单触发价"));
            assert!(reason.contains("tickSz"));
        }
        other => panic!("invalid attached risk trigger should not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "invalid attached risk trigger must reject the open before OKX order submit"
    );
    let row = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-attached-risk-tick-size")
    .fetch_one(&pool)
    .await
    .expect("failed protected open should be persisted");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row
        .get::<String, _>("error_message")
        .contains("保护单触发价"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn unsupported_order_type_is_rejected_before_exchange_side_effects() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_invalid_order_type");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "td_mode": "isolated",
        "leverage": 7,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_invalid_order_type".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-invalid-order-type",
            &config,
            &pool,
            &public_client,
            &private_client,
            &action_record,
            StrategyIntentAction::OpenPosition,
            "maker_or_cancel",
            None,
            None,
            &[],
            None,
            None,
            None,
            true,
        )
        .await;

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("ordType"));
        }
        other => panic!("unsupported order type must not submit: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded
            .iter()
            .all(|request| !request.contains("/api/v5/account/set-leverage")),
        "invalid order type must be rejected before setting leverage: {recorded:#?}"
    );
    assert!(
        recorded
            .iter()
            .all(|request| !request.contains("/api/v5/trade/order")),
        "invalid order type must be rejected before order submit: {recorded:#?}"
    );

    let row = sqlx::query(
        r#"
        SELECT order_type, status, success, error_message
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-invalid-order-type")
    .fetch_one(&pool)
    .await
    .expect("submit failure should be persisted");
    assert_eq!(row.get::<String, _>("order_type"), "maker_or_cancel");
    assert_eq!(row.get::<String, _>("status"), "submit_failed");
    assert_eq!(row.get::<i64, _>("success"), 0);
    assert!(row.get::<String, _>("error_message").contains("ordType"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn okx_submit_rejection_updates_pre_registered_order_without_duplicate_rows() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_order_failure(true).await;
    let db_path = temp_db_path("live_action_submit_rejected");
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
    let mut config = test_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_submit_rejected".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-submit-rejected",
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

    match outcome {
        LiveActionExecutionOutcome::NotSubmitted { reason, .. } => {
            assert!(reason.contains("51000"));
        }
        other => panic!("rejected OKX submit must not be treated as submitted: {other:?}"),
    }
    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded
            .iter()
            .any(|request| request.contains("/api/v5/trade/order")),
        "valid order should reach OKX before exchange rejection"
    );

    let rows = sqlx::query(
        r#"
        SELECT status, success, error_message, client_order_id
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-submit-rejected")
    .fetch_all(&pool)
    .await
    .expect("orders should query");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("status"), "submit_failed");
    assert_eq!(rows[0].get::<i64, _>("success"), 0);
    assert!(rows[0].get::<String, _>("error_message").contains("51000"));
    assert!(!rows[0]
        .get::<String, _>("client_order_id")
        .trim()
        .is_empty());

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
