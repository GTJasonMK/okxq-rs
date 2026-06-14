use super::*;

#[tokio::test]
async fn planned_exit_order_sync_recovers_when_local_order_record_is_missing() {
    let (base_url, stop_server) = start_order_detail_mock_server().await;
    let db_path = temp_db_path("planned_exit_sync_missing_order_record");
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
    mark_live_planned_exit_submitted(&pool, plan.id, "run-exit", "exit-order", "exitclientorder")
        .await
        .expect("plan should be exit_submitted");

    runtime
        .sync_exchange_order_states("run-exit", &config, &pool, &private_client)
        .await;

    let row = sqlx::query(
        "SELECT status, exit_order_id, exit_client_order_id FROM live_execution_plans WHERE id = ?",
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "exit_filled");
    assert_eq!(row.get::<String, _>("exit_order_id"), "exit-order");
    assert_eq!(
        row.get::<String, _>("exit_client_order_id"),
        "exitclientorder"
    );

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn planned_exit_order_sync_uses_order_history_when_active_lookup_is_missing() {
    let (base_url, stop_server) = start_order_not_found_with_history_mock_server().await;
    let db_path = temp_db_path("planned_exit_history_sync");
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
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-exit",
        "history-exit-order",
        "historyexitclient",
    )
    .await
    .expect("plan should be exit_submitted");

    runtime
        .sync_exchange_order_states("run-exit", &config, &pool, &private_client)
        .await;

    let row = sqlx::query(
        r#"
        SELECT status, attempt_count, exit_order_id, exit_client_order_id,
               exit_order_history, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "exit_filled");
    assert_eq!(row.get::<i64, _>("attempt_count"), 0);
    assert_eq!(row.get::<String, _>("exit_order_id"), "history-exit-order");
    assert_eq!(
        row.get::<String, _>("exit_client_order_id"),
        "historyexitclient"
    );
    assert!(row
        .get::<String, _>("exit_order_history")
        .contains("history-exit-order"));
    assert!(row.get::<String, _>("last_error").contains("avgPx=102.25"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn submitted_planned_exit_order_not_found_requeues_plan_for_retry() {
    let (base_url, stop_server) = start_order_not_found_mock_server().await;
    let db_path = temp_db_path("submitted_planned_exit_not_found_requeue");
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
    let order_id = insert_live_exchange_order(
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
        ArrivalQuote::default(),
        "missing-exit-order",
        "missingexitclient",
    )
    .await
    .expect("exit order should insert");
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-exit",
        "missing-exit-order",
        "missingexitclient",
    )
    .await
    .expect("plan should be exit_submitted");
    let stale_created_at = (chrono::Utc::now() - chrono::Duration::seconds(60)).to_rfc3339();
    sqlx::query("UPDATE live_order_records SET created_at = ? WHERE id = ?")
        .bind(stale_created_at)
        .bind(order_id)
        .execute(&pool)
        .await
        .expect("order should become stale enough for not-found handling");

    runtime
        .sync_exchange_order_states("run-exit", &config, &pool, &private_client)
        .await;

    let order_row =
        sqlx::query("SELECT status, success, error_message FROM live_order_records WHERE id = ?")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .expect("order should exist");
    assert_eq!(order_row.get::<String, _>("status"), "rejected");
    assert_eq!(order_row.get::<i64, _>("success"), 0);
    assert!(order_row
        .get::<String, _>("error_message")
        .contains("Order does not exist"));
    let plan_row = sqlx::query(
        "SELECT status, attempt_count, next_attempt_at, last_error FROM live_execution_plans WHERE id = ?",
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(plan_row.get::<String, _>("status"), "scheduled");
    assert_eq!(plan_row.get::<i64, _>("attempt_count"), 1);
    assert!(plan_row.get::<i64, _>("next_attempt_at") > planned_exit.timestamp);
    assert!(plan_row
        .get::<String, _>("last_error")
        .contains("Order does not exist"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn missing_local_planned_exit_order_not_found_requeues_plan_for_retry() {
    let (base_url, stop_server) = start_order_not_found_mock_server().await;
    let db_path = temp_db_path("missing_local_planned_exit_not_found_requeue");
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
    mark_live_planned_exit_submitted(
        &pool,
        plan.id,
        "run-exit",
        "missing-local-exit-order",
        "missinglocalexit",
    )
    .await
    .expect("plan should be exit_submitted");
    let stale_updated_at = (chrono::Utc::now() - chrono::Duration::seconds(60)).to_rfc3339();
    sqlx::query("UPDATE live_execution_plans SET updated_at = ? WHERE id = ?")
        .bind(stale_updated_at)
        .bind(plan.id)
        .execute(&pool)
        .await
        .expect("plan should become stale enough for not-found handling");

    runtime
        .sync_exchange_order_states("run-exit", &config, &pool, &private_client)
        .await;

    let plan_row = sqlx::query(
        "SELECT status, attempt_count, next_attempt_at, last_error FROM live_execution_plans WHERE id = ?",
    )
    .bind(plan.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(plan_row.get::<String, _>("status"), "scheduled");
    assert_eq!(plan_row.get::<i64, _>("attempt_count"), 1);
    assert!(plan_row.get::<i64, _>("next_attempt_at") > planned_exit.timestamp);
    assert!(plan_row
        .get::<String, _>("last_error")
        .contains("Order does not exist"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
