use super::*;

#[test]
fn algo_order_state_maps_effective_and_failed_states() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: "algo-1".to_string(),
        client_order_id: "clalgo1".to_string(),
        status: "algo_submitted".to_string(),
        created_at_ms: 1_780_000_000_000,
    };

    let effective = algo_order_state_from_okx_order(
        &json!({
            "algoId": "algo-1",
            "algoClOrdId": "clalgo1",
            "state": "effective",
            "actualSz": "1"
        }),
        &candidate,
    )
    .expect("effective algo state should map");
    assert_eq!(effective.status, "algo_effective");
    assert!(effective.success);
    assert!(effective.error_message.contains("triggered"));

    let failed = algo_order_state_from_okx_order(
        &json!({
            "algoId": "algo-1",
            "algoClOrdId": "clalgo1",
            "state": "order_failed",
            "failCode": "51000",
            "failReason": "unit test"
        }),
        &candidate,
    )
    .expect("failed algo state should map");
    assert_eq!(failed.status, "algo_failed");
    assert!(!failed.success);
    assert!(failed.error_message.contains("51000"));
}

#[test]
fn sync_order_identity_ignores_invalid_client_id_when_order_id_exists() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: "ord-1".to_string(),
        client_order_id: "bad_client_id".to_string(),
        status: "submitted".to_string(),
        created_at_ms: 1_780_000_000_000,
    };

    assert_eq!(
        sync_order_identity(&candidate),
        Some(("ord-1".to_string(), String::new()))
    );
}

#[test]
fn sync_order_identity_rejects_invalid_client_id_without_order_id() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: String::new(),
        client_order_id: "bad_client_id".to_string(),
        status: "submitted".to_string(),
        created_at_ms: 1_780_000_000_000,
    };

    assert_eq!(sync_order_identity(&candidate), None);
}

#[test]
fn live_fill_sync_symbol_limit_defaults_to_multi_symbol_scope_and_clamps() {
    let mut config = test_config();

    assert_eq!(
        live_fill_sync_symbol_limit(&config),
        DEFAULT_LIVE_FILL_SYNC_SYMBOL_LIMIT
    );

    config.params = json!({"live_fill_sync_symbol_limit": "33"});
    assert_eq!(live_fill_sync_symbol_limit(&config), 33);

    config.params = json!({"fill_sync_symbol_limit": 4});
    assert_eq!(live_fill_sync_symbol_limit(&config), 4);

    config.params = json!({"live_fill_sync_symbol_limit": 200});
    assert_eq!(
        live_fill_sync_symbol_limit(&config),
        MAX_LIVE_FILL_SYNC_SYMBOL_LIMIT
    );

    config.params = json!({"live_fill_sync_symbol_limit": -5});
    assert_eq!(live_fill_sync_symbol_limit(&config), 1);

    config.params = json!({"live_fill_sync_symbol_limit": "NaN"});
    assert_eq!(
        live_fill_sync_symbol_limit(&config),
        DEFAULT_LIVE_FILL_SYNC_SYMBOL_LIMIT
    );
}

#[test]
fn submit_unknown_not_found_waits_for_grace_period_before_rejecting() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: String::new(),
        client_order_id: "clpending".to_string(),
        status: "submit_unknown".to_string(),
        created_at_ms: 1_780_000_000_000,
    };
    let error = "OKX private API error 51603: Order does not exist";

    assert!(!is_exchange_order_not_found_after_grace(
        &candidate,
        error,
        1_780_000_010_000,
    ));
    assert!(is_exchange_order_not_found_after_grace(
        &candidate,
        error,
        1_780_000_030_000,
    ));
    assert!(is_exchange_order_not_found_after_grace(
        &LiveOrderSyncCandidate {
            status: "submitted".to_string(),
            ..candidate.clone()
        },
        error,
        1_780_000_030_000,
    ));
    assert!(!is_exchange_order_not_found_after_grace(
        &LiveOrderSyncCandidate {
            status: "cancel_requested".to_string(),
            ..candidate.clone()
        },
        error,
        1_780_000_030_000,
    ));
}

#[test]
fn okx_order_state_maps_filled_order_to_terminal_success() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: "ord-local".to_string(),
        client_order_id: "cllocal".to_string(),
        status: "submitted".to_string(),
        created_at_ms: 1_780_000_000_000,
    };

    let update = exchange_order_state_from_okx_order(
        &json!({
            "ordId": "ord-exchange",
            "clOrdId": "clexchange",
            "state": "filled",
            "avgPx": "100.5",
            "accFillSz": "3"
        }),
        &candidate,
    )
    .expect("filled order should normalize");

    assert_eq!(update.status, "filled");
    assert!(update.success);
    assert_eq!(update.order_id, "ord-exchange");
    assert_eq!(update.client_order_id, "clexchange");
    assert!(update.error_message.contains("avgPx=100.5"));
}

#[test]
fn okx_order_state_keeps_canceled_partial_fill_as_successful_for_fill_attribution() {
    let candidate = LiveOrderSyncCandidate {
        id: 1,
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        order_id: "ord-local".to_string(),
        client_order_id: "cllocal".to_string(),
        status: "submitted".to_string(),
        created_at_ms: 1_780_000_000_000,
    };

    let update = exchange_order_state_from_okx_order(
        &json!({
            "ordId": "ord-exchange",
            "clOrdId": "clexchange",
            "state": "canceled",
            "avgPx": "100.5",
            "accFillSz": "0.7"
        }),
        &candidate,
    )
    .expect("canceled partial order should normalize");

    assert_eq!(update.status, "canceled");
    assert!(update.success);
    assert!(update.error_message.contains("canceled after partial fill"));
}
