use super::*;

#[tokio::test]
async fn successful_swap_open_persists_planned_exit_plan() {
    let (base_url, stop_server, _requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_planned_exit_plan");
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
        reason: "unit_test_planned_exit".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    runtime
        .evaluate_live_action(
            "run-planned-exit-entry",
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
            Some(&planned_exit),
            None,
            None,
            true,
        )
        .await;

    let order = sqlx::query(
        r#"
        SELECT status, success
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-planned-exit-entry")
    .fetch_one(&pool)
    .await
    .expect("submitted entry order should be persisted");
    assert_eq!(order.get::<String, _>("status"), "submitted");
    assert_eq!(order.get::<i64, _>("success"), 1);

    let plan = sqlx::query(
        r#"
        SELECT status, entry_run_id, symbol, entry_side, close_side,
               planned_exit_time, planned_exit_reason, planned_exit_contract
        FROM live_execution_plans
        WHERE entry_run_id = ?
        "#,
    )
    .bind("run-planned-exit-entry")
    .fetch_one(&pool)
    .await
    .expect("planned exit should be persisted");
    assert_eq!(plan.get::<String, _>("status"), "scheduled");
    assert_eq!(plan.get::<String, _>("symbol"), "BTC-USDT-SWAP");
    assert_eq!(plan.get::<String, _>("entry_side"), "buy");
    assert_eq!(plan.get::<String, _>("close_side"), "sell");
    assert_eq!(
        plan.get::<i64, _>("planned_exit_time"),
        planned_exit.timestamp
    );
    assert_eq!(
        plan.get::<String, _>("planned_exit_reason"),
        "hold_bars_elapsed"
    );
    assert_eq!(
        plan.get::<String, _>("planned_exit_contract"),
        "planned_exit_time_v1"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_submits_okx_close_order_and_marks_plan_submitted() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_order_responses(vec![
            mock_okx_order_submit_response("submitted-1"),
            mock_okx_order_submit_response("planned-exit-close-1"),
        ])
        .await;
    let db_path = temp_db_path("live_action_due_planned_exit_close");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_due_planned_exit".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-planned-exit-entry-due",
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
            Some(&planned_exit),
            None,
            None,
            true,
        )
        .await;
    assert_eq!(outcome, LiveActionExecutionOutcome::Submitted);
    let entry_identity = sqlx::query(
        r#"
        SELECT entry_order_id, entry_client_order_id
        FROM live_execution_plans
        WHERE entry_run_id = ?
        "#,
    )
    .bind("run-planned-exit-entry-due")
    .fetch_one(&pool)
    .await
    .expect("planned exit should store entry order identity");
    insert_test_local_fill(
        &pool,
        "entry-fill-due",
        &config.symbol,
        "buy",
        "2",
        &entry_identity.get::<String, _>("entry_order_id"),
        &entry_identity.get::<String, _>("entry_client_order_id"),
        &config.mode,
    )
    .await;
    requests.lock().expect("recorded requests").clear();

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-due",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    let close_order_request = recorded
        .iter()
        .find(|request| is_exchange_order_request(request))
        .expect("planned exit should submit a close order");
    let close_order_body =
        request_json_body(close_order_request).expect("close order should have JSON body");
    assert_eq!(close_order_body["instId"], "BTC-USDT-SWAP");
    assert_eq!(close_order_body["tdMode"], "cross");
    assert_eq!(close_order_body["side"], "sell");
    assert_eq!(close_order_body["ordType"], "market");
    assert_eq!(close_order_body["sz"], "2");
    assert_eq!(close_order_body["posSide"], "long");
    assert!(close_order_body.get("reduceOnly").is_none());
    assert!(close_order_body
        .get("clOrdId")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty()));

    let plan = sqlx::query(
        r#"
        SELECT status, exit_run_id, exit_order_id, exit_client_order_id,
               attempt_count, last_error
        FROM live_execution_plans
        WHERE entry_run_id = ?
        "#,
    )
    .bind("run-planned-exit-entry-due")
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "exit_submitted");
    assert_eq!(
        plan.get::<String, _>("exit_run_id"),
        "run-planned-exit-close-due"
    );
    assert_eq!(
        plan.get::<String, _>("exit_order_id"),
        "planned-exit-close-1"
    );
    assert!(!plan
        .get::<String, _>("exit_client_order_id")
        .trim()
        .is_empty());
    assert_eq!(plan.get::<i64, _>("attempt_count"), 0);
    assert!(plan.get::<String, _>("last_error").trim().is_empty());

    let exit_order = sqlx::query(
        r#"
        SELECT side, size, status, success, action, action_timestamp
        FROM live_order_records
        WHERE run_id = ?
          AND action = 'close_position'
        "#,
    )
    .bind("run-planned-exit-close-due")
    .fetch_one(&pool)
    .await
    .expect("planned exit close order should be persisted");
    assert_eq!(exit_order.get::<String, _>("side"), "sell");
    assert_eq!(exit_order.get::<f64, _>("size"), 2.0);
    assert_eq!(exit_order.get::<String, _>("status"), "submitted");
    assert_eq!(exit_order.get::<i64, _>("success"), 1);
    assert_eq!(
        exit_order.get::<i64, _>("action_timestamp"),
        planned_exit_time
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_cancels_limit_entry_remainder_before_close() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_due_planned_exit_cancel_entry_remainder");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let entry_signal = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_cancel_entry_remainder".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    insert_live_exchange_order(
        &pool,
        &config,
        "buy",
        "limit",
        4.0,
        100.0,
        "open_position",
        "partially_filled",
        true,
        "entry partially filled",
        "run-planned-exit-entry-limit-remainder",
        entry_signal.timestamp,
        ArrivalQuote::default(),
        "entry-order-limit-remainder",
        "entryclientlimitremainder",
    )
    .await
    .expect("local limit entry order should insert");
    let inserted = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-planned-exit-entry-limit-remainder",
        &entry_signal,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-limit-remainder",
        "entryclientlimitremainder",
    )
    .await
    .expect("planned exit should insert");
    insert_test_local_fill(
        &pool,
        "entry-fill-limit-remainder",
        &config.symbol,
        "buy",
        "2",
        "entry-order-limit-remainder",
        "entryclientlimitremainder",
        &config.mode,
    )
    .await;

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-limit-remainder",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    let cancel_index = recorded
        .iter()
        .position(|request| request.contains("/api/v5/trade/cancel-order"))
        .expect("planned exit should cancel the unfilled entry remainder");
    let close_index = recorded
        .iter()
        .position(|request| is_exchange_order_request(request))
        .expect("planned exit should submit residual close order");
    assert!(
        cancel_index < close_index,
        "entry remainder must be cancelled before the planned close is submitted"
    );
    let cancel_body =
        request_json_body(&recorded[cancel_index]).expect("cancel order should have JSON body");
    assert_eq!(cancel_body["instId"], "BTC-USDT-SWAP");
    assert_eq!(cancel_body["ordId"], "entry-order-limit-remainder");
    assert_eq!(cancel_body["clOrdId"], "entryclientlimitremainder");
    let close_body =
        request_json_body(&recorded[close_index]).expect("close order should have JSON body");
    assert_eq!(close_body["side"], "sell");
    assert_eq!(close_body["sz"], "2");

    let entry_order = sqlx::query(
        r#"
        SELECT status, success, error_message
        FROM live_order_records
        WHERE order_id = ?
        "#,
    )
    .bind("entry-order-limit-remainder")
    .fetch_one(&pool)
    .await
    .expect("entry order should exist");
    assert_eq!(entry_order.get::<String, _>("status"), "cancel_requested");
    assert_eq!(entry_order.get::<i64, _>("success"), 1);
    assert!(entry_order
        .get::<String, _>("error_message")
        .contains("撤单请求已提交"));

    let plan = sqlx::query(
        r#"
        SELECT status, exit_run_id
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(inserted.id)
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "exit_submitted");
    assert_eq!(
        plan.get::<String, _>("exit_run_id"),
        "run-planned-exit-close-limit-remainder"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exits_same_scope_submit_per_entry_residual_close_orders() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_due_planned_exit_grouped_close");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let action_record = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_grouped_due_planned_exit".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let first = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-grouped-a",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-grouped-a",
        "entryclientgroupeda",
    )
    .await
    .expect("first plan should insert");
    let second = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-grouped-b",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-grouped-b",
        "entryclientgroupedb",
    )
    .await
    .expect("second plan should insert");
    insert_test_local_fill(
        &pool,
        "entry-fill-grouped-a",
        &config.symbol,
        "buy",
        "1.5",
        "entry-order-grouped-a",
        "entryclientgroupeda",
        &config.mode,
    )
    .await;
    insert_test_local_fill(
        &pool,
        "entry-fill-grouped-b",
        &config.symbol,
        "buy",
        "0.75",
        "entry-order-grouped-b",
        "entryclientgroupedb",
        &config.mode,
    )
    .await;

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-grouped",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    let close_order_requests = recorded
        .iter()
        .filter(|request| is_exchange_order_request(request))
        .collect::<Vec<_>>();
    assert_eq!(
        close_order_requests.len(),
        2,
        "same-scope planned exits should submit one residual close order per entry plan"
    );
    let close_order_sizes = close_order_requests
        .iter()
        .map(|request| {
            request_json_body(request)
                .and_then(|body| {
                    body.get("sz")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .expect("close order should include size")
        })
        .collect::<Vec<_>>();
    assert!(close_order_sizes.contains(&"1.5".to_string()));
    assert!(close_order_sizes.contains(&"0.75".to_string()));
    let rows = sqlx::query(
        r#"
        SELECT id, status, exit_run_id, exit_order_id, exit_client_order_id
        FROM live_execution_plans
        WHERE id IN (?, ?)
        ORDER BY id ASC
        "#,
    )
    .bind(first.id)
    .bind(second.id)
    .fetch_all(&pool)
    .await
    .expect("planned exits should query");
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.get::<String, _>("status"), "exit_submitted");
        assert_eq!(
            row.get::<String, _>("exit_run_id"),
            "run-planned-exit-close-grouped"
        );
        assert_eq!(row.get::<String, _>("exit_order_id"), "submitted-1");
        assert!(!row
            .get::<String, _>("exit_client_order_id")
            .trim()
            .is_empty());
    }

    let exit_order_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM live_order_records
        WHERE run_id = ?
          AND action = 'close_position'
        "#,
    )
    .bind("run-planned-exit-close-grouped")
    .fetch_one(&pool)
    .await
    .expect("exit order count should query");
    assert_eq!(exit_order_count, 2);

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_waits_for_entry_fill_evidence_before_close() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_due_planned_exit_wait_entry_fill");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let entry_signal = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_wait_entry_fill".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let inserted = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-planned-exit-entry-wait-fill",
        &entry_signal,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-wait-fill",
        "entryclientwaitfill",
    )
    .await
    .expect("planned exit should insert");

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-wait-fill",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| is_exchange_order_request(request)),
        "planned exit without entry fill evidence must not submit a close order"
    );
    let plan = sqlx::query(
        r#"
        SELECT status, exit_run_id, attempt_count, next_attempt_at, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(inserted.id)
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "scheduled");
    assert!(plan.get::<String, _>("exit_run_id").trim().is_empty());
    assert_eq!(plan.get::<i64, _>("attempt_count"), 1);
    assert!(plan.get::<i64, _>("next_attempt_at") > planned_exit_time);
    assert!(plan
        .get::<String, _>("last_error")
        .contains("尚未同步到入口订单成交"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn retried_planned_exit_caps_close_by_entry_residual_after_prior_exit_fill() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_due_planned_exit_retry_residual");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 2_000;
    let entry_signal = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_retry_residual".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let inserted = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-planned-exit-entry-retry-residual",
        &entry_signal,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-retry-residual",
        "entryclientretryresidual",
    )
    .await
    .expect("planned exit should insert");
    insert_test_local_fill(
        &pool,
        "entry-fill-retry-residual",
        &config.symbol,
        "buy",
        "2",
        "entry-order-retry-residual",
        "entryclientretryresidual",
        &config.mode,
    )
    .await;
    mark_live_planned_exit_submitted(
        &pool,
        inserted.id,
        "run-planned-exit-close-old",
        "exit-order-old",
        "exitclientold",
    )
    .await
    .expect("old planned exit submit should persist");
    insert_test_local_fill(
        &pool,
        "exit-fill-old-partial",
        &config.symbol,
        "sell",
        "0.75",
        "exit-order-old",
        "exitclientold",
        &config.mode,
    )
    .await;
    crate::live_strategy::storage::mark_live_planned_exit_order_terminal(
        &pool,
        "exit-order-old",
        "exitclientold",
        "canceled",
        "old planned exit canceled after partial fill",
        chrono::Utc::now().timestamp_millis() - 1_000,
    )
    .await
    .expect("old canceled exit should requeue plan");
    requests.lock().expect("recorded requests").clear();

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-retry-residual",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    let close_order_request = recorded
        .iter()
        .find(|request| is_exchange_order_request(request))
        .expect("planned exit retry should submit residual close order");
    let close_order_body =
        request_json_body(close_order_request).expect("close order should have JSON body");
    assert_eq!(close_order_body["side"], "sell");
    assert_eq!(close_order_body["sz"], "1.25");

    let plan = sqlx::query(
        r#"
        SELECT status, exit_order_history
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(inserted.id)
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "exit_submitted");
    let history = plan.get::<String, _>("exit_order_history");
    assert!(history.contains("exit-order-old"));
    assert!(history.contains("exitclientold"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn explicit_close_position_size_caps_okx_close_quantity() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_explicit_close_position_size_cap");
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
    config.initial_capital = 100.0;
    config.params = json!({
        "contract_mode": true,
        "leverage": 1,
    });
    let action_record = StrategyActionRecord {
        action: "close_position".to_string(),
        side: "flat".to_string(),
        price: 100.0,
        reason: "unit_test_partial_close".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.01),
    };

    let outcome = runtime
        .evaluate_live_action(
            "run-explicit-close-cap",
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
    let close_order_request = recorded
        .iter()
        .find(|request| is_exchange_order_request(request))
        .expect("explicit close should submit a close order");
    let close_order_body =
        request_json_body(close_order_request).expect("close order should have JSON body");
    assert_eq!(close_order_body["instId"], "BTC-USDT-SWAP");
    assert_eq!(close_order_body["side"], "sell");
    assert_eq!(close_order_body["ordType"], "market");
    assert_eq!(close_order_body["sz"], "1");
    assert_eq!(close_order_body["posSide"], "long");

    let order = sqlx::query(
        r#"
        SELECT side, size, status, success, action
        FROM live_order_records
        WHERE run_id = ?
        "#,
    )
    .bind("run-explicit-close-cap")
    .fetch_one(&pool)
    .await
    .expect("explicit close order should be persisted");
    assert_eq!(order.get::<String, _>("side"), "sell");
    assert_eq!(order.get::<f64, _>("size"), 1.0);
    assert_eq!(order.get::<String, _>("status"), "submitted");
    assert_eq!(order.get::<i64, _>("success"), 1);
    assert_eq!(order.get::<String, _>("action"), "close_position");

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_without_position_retries_instead_of_skipping() {
    let (base_url, stop_server, requests) = start_recording_mock_okx_server().await;
    let db_path = temp_db_path("live_action_due_planned_exit_no_position_retry");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let entry_signal = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_due_no_position_retry".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let inserted = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-planned-exit-entry-no-position",
        &entry_signal,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-no-position",
        "entryclientnoposition",
    )
    .await
    .expect("planned exit should insert");
    insert_test_local_fill(
        &pool,
        "entry-fill-no-position",
        &config.symbol,
        "buy",
        "1",
        "entry-order-no-position",
        "entryclientnoposition",
        &config.mode,
    )
    .await;

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-no-position",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        !recorded
            .iter()
            .any(|request| is_exchange_order_request(request)),
        "planned exit without a closeable position must not submit a close order"
    );
    let plan = sqlx::query(
        r#"
        SELECT status, exit_run_id, attempt_count, next_attempt_at, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(inserted.id)
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "scheduled");
    assert!(plan.get::<String, _>("exit_run_id").trim().is_empty());
    assert_eq!(plan.get::<i64, _>("attempt_count"), 1);
    assert!(plan.get::<i64, _>("next_attempt_at") > planned_exit_time);
    let last_error = plan.get::<String, _>("last_error");
    assert!(last_error.contains("继续等待交易所持仓/成交同步"));
    assert!(last_error.contains("当前没有"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_unknown_submit_stays_syncable_without_retrying() {
    let (base_url, stop_server, requests) =
        start_recording_mock_okx_server_with_order_responses(vec!["not-json".to_string()]).await;
    let db_path = temp_db_path("live_action_due_planned_exit_unknown_submit");
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
    let planned_exit_time = chrono::Utc::now().timestamp_millis() - 1_000;
    let entry_signal = StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_due_unknown_submit".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: None,
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: planned_exit_time,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let inserted = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-planned-exit-entry-unknown",
        &entry_signal,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-unknown",
        "entryclientunknown",
    )
    .await
    .expect("planned exit should insert");
    insert_test_local_fill(
        &pool,
        "entry-fill-unknown-submit",
        &config.symbol,
        "buy",
        "1",
        "entry-order-unknown",
        "entryclientunknown",
        &config.mode,
    )
    .await;

    runtime
        .process_due_planned_exits(
            "run-planned-exit-close-unknown",
            &config,
            &pool,
            &public_client,
            &private_client,
        )
        .await;

    let recorded = requests.lock().expect("recorded requests").clone();
    assert!(
        recorded
            .iter()
            .any(|request| is_exchange_order_request(request)),
        "planned exit close order should be submitted before response decode failure"
    );

    let plan = sqlx::query(
        r#"
        SELECT status, exit_run_id, exit_order_id, exit_client_order_id,
               attempt_count, last_error, next_attempt_at
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(inserted.id)
    .fetch_one(&pool)
    .await
    .expect("planned exit should exist");
    assert_eq!(plan.get::<String, _>("status"), "exit_submitted");
    assert_eq!(
        plan.get::<String, _>("exit_run_id"),
        "run-planned-exit-close-unknown"
    );
    assert!(plan.get::<String, _>("exit_order_id").trim().is_empty());
    let exit_client_order_id = plan.get::<String, _>("exit_client_order_id");
    assert!(!exit_client_order_id.trim().is_empty());
    assert_eq!(plan.get::<i64, _>("attempt_count"), 0);
    assert_eq!(plan.get::<i64, _>("next_attempt_at"), 0);
    assert!(plan.get::<String, _>("last_error").trim().is_empty());

    let order = sqlx::query(
        r#"
        SELECT status, success, order_id, client_order_id, error_message
        FROM live_order_records
        WHERE run_id = ?
          AND action = 'close_position'
        "#,
    )
    .bind("run-planned-exit-close-unknown")
    .fetch_one(&pool)
    .await
    .expect("planned exit close order should be persisted");
    assert_eq!(order.get::<String, _>("status"), "submit_unknown");
    assert_eq!(order.get::<i64, _>("success"), 0);
    assert!(order.get::<String, _>("order_id").trim().is_empty());
    assert_eq!(
        order.get::<String, _>("client_order_id"),
        exit_client_order_id
    );
    assert!(order
        .get::<String, _>("error_message")
        .contains("待同步确认"));

    let candidates = crate::live_strategy::storage::query_live_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect("sync candidates should query");
    assert!(candidates.iter().any(|candidate| {
        candidate.client_order_id == exit_client_order_id && candidate.status == "submit_unknown"
    }));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
