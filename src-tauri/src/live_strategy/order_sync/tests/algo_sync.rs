use super::*;

#[tokio::test]
async fn exchange_order_sync_updates_standalone_algo_order_state() {
    let (base_url, stop_server) = start_order_detail_mock_server().await;
    let db_path = temp_db_path("algo_order_sync_updates_state");
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
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitted",
        true,
        "algo submitted",
        "run-algo-sync",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "algo-live",
        "clalgolive",
    )
    .await
    .expect("algo order should insert");

    runtime
        .sync_exchange_order_states("run-algo-sync", &config, &pool, &private_client)
        .await;

    let row = sqlx::query(
        "SELECT status, success, error_message FROM live_order_records WHERE order_id = ?",
    )
    .bind("algo-live")
    .fetch_one(&pool)
    .await
    .expect("algo order should exist");
    assert_eq!(row.get::<String, _>("status"), "algo_live");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("protective algo order live"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn exchange_order_sync_recovers_pre_registered_algo_order_by_client_id() {
    let (base_url, stop_server) = start_order_detail_mock_server().await;
    let db_path = temp_db_path("algo_order_sync_recovers_pre_registered_by_client_id");
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
        "sell",
        "stop_market",
        1.0,
        94.0,
        "place_risk_order",
        "algo_submitting",
        true,
        "algo submitting",
        "run-algo-sync-client-id",
        1_780_000_000_000,
        ArrivalQuote::default(),
        "",
        "clalgolive",
    )
    .await
    .expect("pre-registered algo order should insert");

    runtime
        .sync_exchange_order_states("run-algo-sync-client-id", &config, &pool, &private_client)
        .await;

    let row = sqlx::query(
        r#"
        SELECT order_id, client_order_id, status, success, error_message
        FROM live_order_records
        WHERE client_order_id = ?
        "#,
    )
    .bind("clalgolive")
    .fetch_one(&pool)
    .await
    .expect("pre-registered algo order should exist");
    assert_eq!(row.get::<String, _>("order_id"), "algo-live");
    assert_eq!(row.get::<String, _>("client_order_id"), "clalgolive");
    assert_eq!(row.get::<String, _>("status"), "algo_live");
    assert_eq!(row.get::<i64, _>("success"), 1);
    assert!(row
        .get::<String, _>("error_message")
        .contains("protective algo order live"));

    let _ = stop_server.send(());
    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}
