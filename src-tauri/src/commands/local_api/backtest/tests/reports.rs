use super::*;

#[tokio::test]
async fn backtest_report_value_skips_or_persists_real_db_rows() {
    let db_path = test_db_path("backtest-persist");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let candles = test_candles(30);
    let report = test_backtest_report(&config, &candles);

    let transient = backtest_report_value(&pool, &report, false).await.unwrap();
    assert!(transient.get("id").is_some_and(Value::is_null));
    assert_eq!(count_backtest_results(&pool).await, 0);

    let persisted = backtest_report_value(&pool, &report, true).await.unwrap();
    assert!(persisted.get("id").and_then(Value::as_i64).unwrap_or(0) > 0);
    assert_eq!(count_backtest_results(&pool).await, 1);

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn backtest_report_value_returns_runtime_integrity_immediately() {
    let db_path = test_db_path("backtest-report-integrity");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let candles = test_candles(30);
    let mut report = test_backtest_report(&config, &candles);
    report.detail["runtime_action_summary"] = json!({
        "planned_exit_contract": "planned_exit_missing",
        "warnings": ["open_actions_missing_planned_exit"]
    });
    report.detail["strategy_actions"] = json!([
        {"action": "open_position"}
    ]);

    let transient = backtest_report_value(&pool, &report, false).await.unwrap();
    let persisted = backtest_report_value(&pool, &report, true).await.unwrap();

    for value in [transient, persisted] {
        let integrity = value
            .get("backtest_result_integrity")
            .expect("run response should expose integrity");
        assert_eq!(
            integrity.get("status").and_then(Value::as_str),
            Some("invalid")
        );
        assert_eq!(
            integrity
                .get("runtime_action_summary_present")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(integrity
            .get("issues")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("planned_exit_missing")));
    }

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn backtest_summary_row_includes_runtime_integrity_from_detail_json() {
    let db_path = test_db_path("backtest-summary-integrity");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let candles = test_candles(30);
    let mut report = test_backtest_report(&config, &candles);
    report.detail["runtime_action_summary"] = json!({
        "planned_exit_contract": "planned_exit_missing",
        "warnings": []
    });
    report.detail["strategy_actions"] = json!([
        {"action": "open_position"}
    ]);

    let persisted = backtest_report_value(&pool, &report, true).await.unwrap();
    let result_id = persisted.get("id").and_then(Value::as_i64).unwrap();
    let row = sqlx::query(
        r#"
        SELECT id, strategy_name, strategy_id, symbol, inst_type, timeframe, days,
               start_time, end_time, initial_capital, final_capital,
               total_return, annual_return, max_drawdown,
               sharpe_ratio, sortino_ratio, calmar_ratio,
               win_rate, profit_factor,
               total_trades, winning_trades, losing_trades,
               avg_profit, avg_loss, largest_profit, largest_loss,
               total_commission, params_json, detail_json, created_at,
               json_type(detail_json, '$.runtime_action_summary') AS runtime_action_summary_type,
               json_array_length(detail_json, '$.strategy_actions') AS strategy_action_count,
               json_array_length(detail_json, '$.runtime_action_summary.warnings') AS runtime_summary_warning_count,
               json_extract(detail_json, '$.runtime_action_summary.planned_exit_contract') AS planned_exit_contract
        FROM backtest_results
        WHERE id = ?
        "#,
    )
    .bind(result_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let summary = backtest_summary_row(row).expect("summary row");
    let integrity = summary
        .get("backtest_result_integrity")
        .expect("summary row should expose integrity");

    assert_eq!(
        integrity.get("status").and_then(Value::as_str),
        Some("invalid")
    );
    assert_eq!(
        integrity
            .get("planned_exit_contract")
            .and_then(Value::as_str),
        Some("planned_exit_missing")
    );
    assert_eq!(
        integrity
            .get("runtime_action_summary_present")
            .and_then(Value::as_bool),
        Some(true)
    );

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn backtest_summary_row_includes_strategy_history_contract_integrity() {
    let db_path = test_db_path("backtest-summary-history-contract-integrity");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let candles = test_candles(30);
    let mut report = test_backtest_report(&config, &candles);
    report.detail["runtime_action_summary"] = json!({
        "planned_exit_contract": "planned_exit_missing",
        "warnings": ["open_actions_missing_planned_exit"]
    });
    report.detail["strategy_diagnostics"] = json!({
        "history_action_contract": {
            "status": "planned_exit_complete",
            "open_action_count": 2,
            "open_actions_with_planned_exit": 2
        }
    });

    let persisted = backtest_report_value(&pool, &report, true).await.unwrap();
    let result_id = persisted.get("id").and_then(Value::as_i64).unwrap();
    let row = sqlx::query(
        r#"
        SELECT id, strategy_name, strategy_id, symbol, inst_type, timeframe, days,
               start_time, end_time, initial_capital, final_capital,
               total_return, annual_return, max_drawdown,
               sharpe_ratio, sortino_ratio, calmar_ratio,
               win_rate, profit_factor,
               total_trades, winning_trades, losing_trades,
               avg_profit, avg_loss, largest_profit, largest_loss,
               total_commission, params_json, detail_json, created_at,
               json_type(detail_json, '$.runtime_action_summary') AS runtime_action_summary_type,
               json_array_length(detail_json, '$.strategy_actions') AS strategy_action_count,
               json_array_length(detail_json, '$.runtime_action_summary.warnings') AS runtime_summary_warning_count,
               json_extract(detail_json, '$.runtime_action_summary.planned_exit_contract') AS planned_exit_contract,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.status') AS history_action_contract_status,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.open_action_count') AS history_open_action_count,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.open_actions_with_planned_exit') AS history_open_actions_with_planned_exit
        FROM backtest_results
        WHERE id = ?
        "#,
    )
    .bind(result_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let summary = backtest_summary_row(row).expect("summary row");
    let integrity = summary
        .get("backtest_result_integrity")
        .expect("summary row should expose strategy contract integrity");

    assert_eq!(
        integrity.get("status").and_then(Value::as_str),
        Some("invalid")
    );
    assert_eq!(
        integrity
            .get("history_action_contract_status")
            .and_then(Value::as_str),
        Some("planned_exit_complete")
    );
    assert!(integrity
        .get("issues")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some("strategy_runtime_action_contract_mismatch")));

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn backtest_summary_row_does_not_short_circuit_historical_live_integrity() {
    let db_path = test_db_path("backtest-summary-historical-live-integrity");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();
    let config = test_strategy_config();
    let candles = test_candles(30);
    let mut report = test_backtest_report(&config, &candles);
    report.detail["engine_version"] = json!("historical_live_v1");
    report.detail["runtime_action_summary"] = json!({
        "planned_exit_contract": "planned_exit_missing",
        "warnings": []
    });

    let persisted = backtest_report_value(&pool, &report, true).await.unwrap();
    let result_id = persisted.get("id").and_then(Value::as_i64).unwrap();
    let row = sqlx::query(
        r#"
        SELECT id, strategy_name, strategy_id, symbol, inst_type, timeframe, days,
               start_time, end_time, initial_capital, final_capital,
               total_return, annual_return, max_drawdown,
               sharpe_ratio, sortino_ratio, calmar_ratio,
               win_rate, profit_factor,
               total_trades, winning_trades, losing_trades,
               avg_profit, avg_loss, largest_profit, largest_loss,
               total_commission, params_json, detail_json, created_at,
               json_type(detail_json, '$.runtime_action_summary') AS runtime_action_summary_type,
               json_array_length(detail_json, '$.strategy_actions') AS strategy_action_count,
               json_array_length(detail_json, '$.runtime_action_summary.warnings') AS runtime_summary_warning_count,
               json_extract(detail_json, '$.runtime_action_summary.planned_exit_contract') AS planned_exit_contract,
               json_extract(detail_json, '$.engine_version') AS engine_version
        FROM backtest_results
        WHERE id = ?
        "#,
    )
    .bind(result_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let summary = backtest_summary_row(row).expect("summary row");
    let integrity = summary
        .get("backtest_result_integrity")
        .expect("historical live summary should expose runtime integrity");

    assert_eq!(
        integrity.get("status").and_then(Value::as_str),
        Some("invalid")
    );
    assert_eq!(
        integrity.get("engine_version").and_then(Value::as_str),
        Some("historical_live_v1")
    );
    assert!(integrity
        .get("issues")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some("planned_exit_missing")));

    cleanup_db(pool, &db_path).await;
}

#[tokio::test]
async fn backtest_history_filtered_recent_queries_use_recent_indexes() {
    let db_path = test_db_path("backtest-history-recent-indexes");
    let pool = storage::connect_and_migrate(&db_path).await.unwrap();

    let strategy_plan = query_plan_details(
        &pool,
        r#"
        EXPLAIN QUERY PLAN
        SELECT id FROM backtest_results
        WHERE strategy_id = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
        &[("strategy_id", "test_runtime_strategy"), ("limit", "200")],
    )
    .await;
    let strategy_symbol_plan = query_plan_details(
        &pool,
        r#"
        EXPLAIN QUERY PLAN
        SELECT id FROM backtest_results
        WHERE strategy_id = ? AND symbol = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
        &[
            ("strategy_id", "test_runtime_strategy"),
            ("symbol", "BTC-USDT-SWAP"),
            ("limit", "200"),
        ],
    )
    .await;

    assert!(
        strategy_plan
            .iter()
            .any(|detail| detail.contains("idx_backtest_strategy_recent")),
        "strategy filter should use recent index, got {strategy_plan:?}"
    );
    assert!(
        strategy_symbol_plan
            .iter()
            .any(|detail| detail.contains("idx_backtest_strategy_symbol_recent")),
        "strategy+symbol filter should use recent index, got {strategy_symbol_plan:?}"
    );
    assert!(
        strategy_plan
            .iter()
            .chain(strategy_symbol_plan.iter())
            .all(|detail| !detail.contains("USE TEMP B-TREE")),
        "filtered recent history queries should not sort with temp b-tree: {strategy_plan:?} {strategy_symbol_plan:?}"
    );

    cleanup_db(pool, &db_path).await;
}
