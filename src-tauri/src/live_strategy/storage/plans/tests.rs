use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use super::*;

#[tokio::test]
async fn live_planned_exit_plan_is_due_only_after_exit_time() {
    let db_path = temp_db_path("planned_exit_due");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "hold_bars_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    let outcome = insert_live_planned_exit_plan(
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

    assert!(outcome.inserted);
    assert!(query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        planned_exit.timestamp - 1,
        10,
    )
    .await
    .expect("due query should run")
    .is_empty());

    let due = query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        planned_exit.timestamp,
        10,
    )
    .await
    .expect("due query should run");
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, outcome.id);
    assert_eq!(due[0].close_side, "sell");
    assert_eq!(due[0].planned_exit_reason, "hold_bars_elapsed");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn same_action_planned_exits_are_distinct_per_entry_order_identity() {
    let db_path = temp_db_path("planned_exit_distinct_entry_orders");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    let first = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-a",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order",
        "entry-client-order",
    )
    .await
    .expect("first plan should insert");
    let second = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-b",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-duplicate",
        "entry-client-order-duplicate",
    )
    .await
    .expect("second entry plan should insert");

    assert!(first.inserted);
    assert!(second.inserted);
    assert_ne!(first.id, second.id);

    let duplicate = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-c",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order",
        "entry-client-order",
    )
    .await
    .expect("duplicate entry plan should reuse existing row");
    assert!(!duplicate.inserted);
    assert_eq!(duplicate.id, first.id);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn due_planned_exit_claim_is_single_consumer_and_hides_from_due_query() {
    let db_path = temp_db_path("planned_exit_claim");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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

    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("due plan should claim")
    );
    assert!(
        !claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("claimed plan should not claim twice")
    );
    assert!(query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        planned_exit.timestamp,
        10,
    )
    .await
    .expect("due query should run")
    .is_empty());

    let status: String = sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
        .bind(outcome.id)
        .fetch_one(&pool)
        .await
        .expect("plan should exist");
    assert_eq!(status, "exit_processing");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn claimed_planned_exit_can_submit_or_retry() {
    let db_path = temp_db_path("planned_exit_claim_submit_retry");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let submit_plan = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-submit",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-submit",
        "entry-client-order-submit",
    )
    .await
    .expect("submit plan should insert");
    assert!(
        claim_due_live_planned_exit(&pool, submit_plan.id, planned_exit.timestamp)
            .await
            .expect("submit plan should claim")
    );
    assert!(mark_live_planned_exit_submitting(
        &pool,
        submit_plan.id,
        "run-exit",
        "pre-submit-client-order",
    )
    .await
    .expect("claimed plan should record pre-submit client id"));
    let pre_submit_client_id: String =
        sqlx::query_scalar("SELECT exit_client_order_id FROM live_execution_plans WHERE id = ?")
            .bind(submit_plan.id)
            .fetch_one(&pool)
            .await
            .expect("claimed plan should exist");
    assert_eq!(pre_submit_client_id, "pre-submit-client-order");
    mark_live_planned_exit_submitted(
        &pool,
        submit_plan.id,
        "run-exit",
        "exit-order",
        "exit-client-order",
    )
    .await
    .expect("claimed plan should submit");
    let submitted_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(submit_plan.id)
            .fetch_one(&pool)
            .await
            .expect("submitted plan should exist");
    assert_eq!(submitted_status, "exit_submitted");

    let mut retry_action_record = action_record.clone();
    retry_action_record.timestamp += 1;
    let retry_plan = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-retry",
        &retry_action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-retry",
        "entry-client-order-retry",
    )
    .await
    .expect("retry plan should insert");
    assert!(
        claim_due_live_planned_exit(&pool, retry_plan.id, planned_exit.timestamp)
            .await
            .expect("retry plan should claim")
    );
    mark_live_planned_exit_retry(
        &pool,
        retry_plan.id,
        planned_exit.timestamp + 60_000,
        "temporary close failure",
    )
    .await
    .expect("claimed plan should retry");
    let retry_row = sqlx::query(
        "SELECT status, attempt_count, next_attempt_at FROM live_execution_plans WHERE id = ?",
    )
    .bind(retry_plan.id)
    .fetch_one(&pool)
    .await
    .expect("retry plan should exist");
    assert_eq!(retry_row.get::<String, _>("status"), "scheduled");
    assert_eq!(retry_row.get::<i64, _>("attempt_count"), 1);
    assert_eq!(
        retry_row.get::<i64, _>("next_attempt_at"),
        planned_exit.timestamp + 60_000
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn stale_processing_planned_exit_is_requeued() {
    let db_path = temp_db_path("planned_exit_stale_processing");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("plan should claim")
    );
    sqlx::query("UPDATE live_execution_plans SET updated_at = ? WHERE id = ?")
        .bind("2026-01-01T00:00:00+00:00")
        .bind(outcome.id)
        .execute(&pool)
        .await
        .expect("test should age processing plan");

    let retry_at = planned_exit.timestamp + 30_000;
    let changed = requeue_stale_live_planned_exit_claims(
        &pool,
        &config.mode,
        &config.strategy_id,
        "2026-01-01T00:01:00+00:00",
        retry_at,
    )
    .await
    .expect("stale processing plan should requeue");

    assert_eq!(changed, 1);
    let row = sqlx::query(
        "SELECT status, attempt_count, next_attempt_at, last_error FROM live_execution_plans WHERE id = ?",
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "scheduled");
    assert_eq!(row.get::<i64, _>("attempt_count"), 1);
    assert_eq!(row.get::<i64, _>("next_attempt_at"), retry_at);
    assert!(row.get::<String, _>("last_error").contains("处理超时"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn stale_processing_plan_with_submitted_order_is_recovered_not_requeued() {
    let db_path = temp_db_path("planned_exit_recover_submitted_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("plan should claim")
    );
    age_processing_plan_for_test(&pool, outcome.id).await;
    insert_matching_exit_order_for_test(
        &pool,
        &config,
        "run-exit",
        planned_exit.timestamp,
        "submitted",
        1,
        "exit-order",
        "exit-client-order",
    )
    .await;

    let recovered = recover_stale_live_planned_exit_claims_from_orders(
        &pool,
        &config.mode,
        &config.strategy_id,
        "2026-01-01T00:01:00+00:00",
        planned_exit.timestamp + 30_000,
    )
    .await
    .expect("submitted order should recover plan");
    let requeued = requeue_stale_live_planned_exit_claims(
        &pool,
        &config.mode,
        &config.strategy_id,
        "2026-01-01T00:01:00+00:00",
        planned_exit.timestamp + 30_000,
    )
    .await
    .expect("requeue should skip recovered plan");

    assert_eq!(recovered, 1);
    assert_eq!(requeued, 0);
    let row = sqlx::query(
        "SELECT status, exit_run_id, exit_order_id, exit_client_order_id, attempt_count FROM live_execution_plans WHERE id = ?",
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "exit_submitted");
    assert_eq!(row.get::<String, _>("exit_run_id"), "run-exit");
    assert_eq!(row.get::<String, _>("exit_order_id"), "exit-order");
    assert_eq!(
        row.get::<String, _>("exit_client_order_id"),
        "exit-client-order"
    );
    assert_eq!(row.get::<i64, _>("attempt_count"), 0);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn stale_processing_plan_with_filled_order_recovers_terminal_state() {
    let db_path = temp_db_path("planned_exit_recover_filled_order");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("plan should claim")
    );
    age_processing_plan_for_test(&pool, outcome.id).await;
    insert_matching_exit_order_for_test(
        &pool,
        &config,
        "run-exit",
        planned_exit.timestamp,
        "filled",
        1,
        "exit-order-filled",
        "exit-client-filled",
    )
    .await;

    let recovered = recover_stale_live_planned_exit_claims_from_orders(
        &pool,
        &config.mode,
        &config.strategy_id,
        "2026-01-01T00:01:00+00:00",
        planned_exit.timestamp + 30_000,
    )
    .await
    .expect("filled order should recover terminal plan");

    assert_eq!(recovered, 1);
    let status: String = sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
        .bind(outcome.id)
        .fetch_one(&pool)
        .await
        .expect("plan should exist");
    assert_eq!(status, "exit_filled");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn planned_exit_retry_delays_due_query_then_submit_closes_plan() {
    let db_path = temp_db_path("planned_exit_retry_submit");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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

    mark_live_planned_exit_retry(
        &pool,
        outcome.id,
        planned_exit.timestamp + 60_000,
        "rate limit",
    )
    .await
    .expect("retry marker should update");
    assert!(query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        planned_exit.timestamp + 1,
        10,
    )
    .await
    .expect("due query should run")
    .is_empty());

    let wakeup = next_live_planned_exit_wakeup(&pool, &config.mode, &config.strategy_id)
        .await
        .expect("wakeup query should run");
    assert_eq!(wakeup, Some(planned_exit.timestamp + 60_000));

    mark_live_planned_exit_submitted(
        &pool,
        outcome.id,
        "run-exit",
        "exit-order",
        "exit-client-order",
    )
    .await
    .expect("submitted marker should update");
    assert!(query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        planned_exit.timestamp + 60_000,
        10,
    )
    .await
    .expect("due query should run")
    .is_empty());

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn submitted_planned_exit_advances_to_terminal_exit_state_from_order_status() {
    let db_path = temp_db_path("planned_exit_terminal_order_state");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
        outcome.id,
        "run-exit",
        "exit-order",
        "exit-client-order",
    )
    .await
    .expect("submitted marker should update");

    let changed = mark_live_planned_exit_order_terminal(
        &pool,
        "exit-order",
        "",
        "filled",
        "OKX order filled",
        planned_exit.timestamp + 60_000,
    )
    .await
    .expect("terminal marker should update");

    assert_eq!(changed, 1);
    let status: String = sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
        .bind(outcome.id)
        .fetch_one(&pool)
        .await
        .expect("plan should exist");
    assert_eq!(status, "exit_filled");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn processing_planned_exit_accepts_early_terminal_order_event_by_client_id() {
    let db_path = temp_db_path("planned_exit_processing_early_terminal");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("plan should claim")
    );
    assert!(
        mark_live_planned_exit_submitting(&pool, outcome.id, "run-exit", "early-client")
            .await
            .expect("submitting marker should update")
    );

    let changed = mark_live_planned_exit_order_terminal(
        &pool,
        "early-order",
        "early-client",
        "filled",
        "OKX order filled before local submitted marker",
        planned_exit.timestamp + 60_000,
    )
    .await
    .expect("early terminal marker should update");

    assert_eq!(changed, 1);
    let row = sqlx::query(
        r#"
        SELECT status, exit_order_id, exit_client_order_id, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "exit_filled");
    assert_eq!(row.get::<String, _>("exit_order_id"), "early-order");
    assert_eq!(row.get::<String, _>("exit_client_order_id"), "early-client");
    assert!(row
        .get::<String, _>("last_error")
        .contains("before local submitted marker"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn canceled_planned_exit_order_requeues_plan_for_residual_position_retry() {
    let db_path = temp_db_path("planned_exit_canceled_retry");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
        outcome.id,
        "run-exit",
        "exit-order",
        "exit-client-order",
    )
    .await
    .expect("submitted marker should update");

    let retry_at = planned_exit.timestamp + 60_000;
    let changed = mark_live_planned_exit_order_terminal(
        &pool,
        "exit-order",
        "",
        "canceled",
        "OKX order canceled after partial fill",
        retry_at,
    )
    .await
    .expect("canceled marker should requeue");

    assert_eq!(changed, 1);
    let row = sqlx::query(
        r#"
        SELECT status, attempt_count, next_attempt_at, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "scheduled");
    assert_eq!(row.get::<i64, _>("attempt_count"), 1);
    assert_eq!(row.get::<i64, _>("next_attempt_at"), retry_at);
    assert!(row
        .get::<String, _>("last_error")
        .contains("canceled after partial fill"));
    assert!(query_due_live_planned_exits(
        &pool,
        &config.mode,
        &config.strategy_id,
        retry_at - 1,
        10,
    )
    .await
    .expect("due query should run")
    .is_empty());
    assert_eq!(
        query_due_live_planned_exits(&pool, &config.mode, &config.strategy_id, retry_at, 10,)
            .await
            .expect("due query should run")
            .len(),
        1
    );

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn planned_exit_terminal_update_is_scoped_to_strategy() {
    let db_path = temp_db_path("planned_exit_terminal_strategy_scope");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let mut other_config = test_config();
    other_config.strategy_id = "other_planned_exit_strategy".to_string();
    other_config.strategy_name = "Other Planned Exit Strategy".to_string();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let target = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-target-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "target-entry-order",
        "target-entry-client",
    )
    .await
    .expect("target plan should insert");
    let other = insert_live_planned_exit_plan(
        &pool,
        &other_config,
        "run-other-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "other-entry-order",
        "other-entry-client",
    )
    .await
    .expect("other strategy plan should insert");
    mark_live_planned_exit_submitted(
        &pool,
        target.id,
        "run-target-exit",
        "shared-exit-order",
        "shared-exit-client",
    )
    .await
    .expect("target plan should be submitted");
    mark_live_planned_exit_submitted(
        &pool,
        other.id,
        "run-other-exit",
        "shared-exit-order",
        "shared-exit-client",
    )
    .await
    .expect("other strategy plan should be submitted");

    let changed = mark_live_planned_exit_order_terminal_for_strategy(
        &pool,
        &config.mode,
        &config.strategy_id,
        &config.symbol,
        "shared-exit-order",
        "shared-exit-client",
        "filled",
        "target strategy order filled",
        planned_exit.timestamp + 60_000,
    )
    .await
    .expect("scoped terminal marker should update");

    assert_eq!(changed, 1);
    let target_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(target.id)
            .fetch_one(&pool)
            .await
            .expect("target plan should exist");
    let other_status: String =
        sqlx::query_scalar("SELECT status FROM live_execution_plans WHERE id = ?")
            .bind(other.id)
            .fetch_one(&pool)
            .await
            .expect("other plan should exist");
    assert_eq!(target_status, "exit_filled");
    assert_eq!(other_status, "exit_submitted");

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn planned_exit_mode_scoped_terminal_update_rejects_ambiguous_strategy_matches() {
    let db_path = temp_db_path("planned_exit_terminal_mode_ambiguous");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let mut other_config = test_config();
    other_config.strategy_id = "other_mode_ambiguous_strategy".to_string();
    other_config.strategy_name = "Other Mode Ambiguous Strategy".to_string();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let first = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-first-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "first-entry-order",
        "first-entry-client",
    )
    .await
    .expect("first plan should insert");
    let second = insert_live_planned_exit_plan(
        &pool,
        &other_config,
        "run-second-entry",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "second-entry-order",
        "second-entry-client",
    )
    .await
    .expect("second strategy plan should insert");
    for plan_id in [first.id, second.id] {
        mark_live_planned_exit_submitted(
            &pool,
            plan_id,
            "run-shared-exit",
            "ambiguous-exit-order",
            "ambiguous-exit-client",
        )
        .await
        .expect("plan should be submitted");
    }

    let error = mark_live_planned_exit_order_terminal_for_mode(
        &pool,
        &config.mode,
        &config.symbol,
        "ambiguous-exit-order",
        "ambiguous-exit-client",
        "filled",
        "ambiguous order filled",
        planned_exit.timestamp + 60_000,
    )
    .await
    .expect_err("mode-scoped terminal marker must reject ambiguous plans");

    assert!(error.to_string().contains("命中多个计划退出记录"));
    let statuses = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status
        FROM live_execution_plans
        WHERE id IN (?, ?)
        ORDER BY id ASC
        "#,
    )
    .bind(first.id)
    .bind(second.id)
    .fetch_one(&pool)
    .await
    .expect("statuses should query");
    assert_eq!(statuses, "exit_submitted");
    let unchanged_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM live_execution_plans
        WHERE id IN (?, ?)
          AND status = 'exit_submitted'
        "#,
    )
    .bind(first.id)
    .bind(second.id)
    .fetch_one(&pool)
    .await
    .expect("unchanged count should query");
    assert_eq!(unchanged_count, 2);

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn processing_planned_exit_accepts_early_canceled_order_event_by_client_id() {
    let db_path = temp_db_path("planned_exit_processing_early_canceled");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
    assert!(
        claim_due_live_planned_exit(&pool, outcome.id, planned_exit.timestamp)
            .await
            .expect("plan should claim")
    );
    assert!(mark_live_planned_exit_submitting(
        &pool,
        outcome.id,
        "run-exit",
        "early-cancel-client"
    )
    .await
    .expect("submitting marker should update"));

    let retry_at = planned_exit.timestamp + 60_000;
    let changed = mark_live_planned_exit_order_terminal(
        &pool,
        "early-cancel-order",
        "early-cancel-client",
        "canceled",
        "OKX order canceled before local submitted marker",
        retry_at,
    )
    .await
    .expect("early canceled marker should requeue");

    assert_eq!(changed, 1);
    let row = sqlx::query(
        r#"
        SELECT status, exit_order_id, exit_client_order_id, attempt_count, next_attempt_at, last_error
        FROM live_execution_plans
        WHERE id = ?
        "#,
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .expect("plan should exist");
    assert_eq!(row.get::<String, _>("status"), "scheduled");
    assert_eq!(row.get::<String, _>("exit_order_id"), "early-cancel-order");
    assert_eq!(
        row.get::<String, _>("exit_client_order_id"),
        "early-cancel-client"
    );
    assert_eq!(row.get::<i64, _>("attempt_count"), 1);
    assert_eq!(row.get::<i64, _>("next_attempt_at"), retry_at);
    assert!(row
        .get::<String, _>("last_error")
        .contains("before local submitted marker"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_execution_plans_returns_current_run_plan_states() {
    let db_path = temp_db_path("planned_exit_query_states");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
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
        outcome.id,
        "run-exit",
        "exit-order",
        "exit-client-order",
    )
    .await
    .expect("submitted marker should update");

    let entry_plans = query_live_execution_plans(&pool, 10, &config.mode, "run-entry")
        .await
        .expect("entry run plans should query");
    let exit_plans = query_live_execution_plans(&pool, 10, &config.mode, "run-exit")
        .await
        .expect("exit run plans should query");
    let unrelated = query_live_execution_plans(&pool, 10, &config.mode, "run-other")
        .await
        .expect("unrelated run plans should query");

    assert_eq!(entry_plans.len(), 1);
    assert_eq!(exit_plans.len(), 1);
    assert!(unrelated.is_empty());
    assert_eq!(entry_plans[0]["id"].as_i64(), Some(outcome.id));
    assert_eq!(entry_plans[0]["status"].as_str(), Some("exit_submitted"));
    assert_eq!(entry_plans[0]["entry_run_id"].as_str(), Some("run-entry"));
    assert_eq!(entry_plans[0]["exit_run_id"].as_str(), Some("run-exit"));
    assert_eq!(
        entry_plans[0]["planned_exit_time"].as_i64(),
        Some(planned_exit.timestamp)
    );
    assert_eq!(
        entry_plans[0]["entry_price"].as_f64(),
        Some(action_record.price)
    );
    assert_eq!(entry_plans[0]["exit_order_id"].as_str(), Some("exit-order"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_live_execution_plans_rejects_dirty_required_timestamp() {
    let db_path = temp_db_path("planned_exit_query_dirty_timestamp");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-dirty-plan",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-dirty",
        "entry-client-dirty",
    )
    .await
    .expect("plan should insert");
    sqlx::query("UPDATE live_execution_plans SET entry_action_timestamp = ? WHERE id = ?")
        .bind("bad-entry-ts")
        .bind(outcome.id)
        .execute(&pool)
        .await
        .expect("dirty timestamp should update");

    let error = query_live_execution_plans(&pool, 10, &config.mode, "run-dirty-plan")
        .await
        .expect_err("dirty required plan timestamp should fail fast");

    assert!(error.to_string().contains("entry_action_timestamp"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

#[tokio::test]
async fn query_submitted_live_planned_exit_order_sync_candidates_rejects_dirty_updated_at() {
    let db_path = temp_db_path("planned_exit_sync_dirty_updated_at");
    let pool = crate::storage::connect_and_migrate(&db_path)
        .await
        .expect("test database should migrate");
    let config = test_config();
    let action_record = test_action_record();
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1_780_000_900_000,
        reason: "close_position".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let outcome = insert_live_planned_exit_plan(
        &pool,
        &config,
        "run-entry-dirty-updated-at",
        &action_record,
        "buy",
        "sell",
        &planned_exit,
        "entry-order-dirty-updated-at",
        "entry-client-dirty-updated-at",
    )
    .await
    .expect("plan should insert");
    mark_live_planned_exit_submitted(
        &pool,
        outcome.id,
        "run-exit-dirty-updated-at",
        "exit-order-dirty-updated-at",
        "exit-client-dirty-updated-at",
    )
    .await
    .expect("submitted marker should update");
    sqlx::query("UPDATE live_execution_plans SET updated_at = ? WHERE id = ?")
        .bind("not-a-timestamp")
        .bind(outcome.id)
        .execute(&pool)
        .await
        .expect("dirty updated_at should update");

    let error = query_submitted_live_planned_exit_order_sync_candidates(
        &pool,
        &config.mode,
        &config.strategy_id,
        10,
    )
    .await
    .expect_err("dirty updated_at should fail fast");

    assert!(error.to_string().contains("updated_at_ms"));

    pool.close().await;
    if let Some(parent) = db_path.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
}

fn test_config() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "planned_exit_storage_test".to_string(),
        strategy_name: "Planned Exit Storage Test".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1000.0,
        position_size: 0.2,
        stop_loss: 0.0,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({}),
        project_root: PathBuf::from("."),
        risk_control_enabled: false,
        max_single_loss_ratio: 0.0,
        max_position_pct: 0.0,
        max_order_value: 0.0,
    }
}

fn test_action_record() -> StrategyActionRecord {
    StrategyActionRecord {
        action: "open_position".to_string(),
        side: "buy".to_string(),
        price: 100.0,
        reason: "unit_test_entry".to_string(),
        strength: 1.0,
        timestamp: 1_780_000_000_000,
        position_size: Some(0.1),
    }
}

async fn age_processing_plan_for_test(pool: &SqlitePool, plan_id: i64) {
    sqlx::query("UPDATE live_execution_plans SET updated_at = ? WHERE id = ?")
        .bind("2026-01-01T00:00:00+00:00")
        .bind(plan_id)
        .execute(pool)
        .await
        .expect("test should age processing plan");
}

#[allow(clippy::too_many_arguments)]
async fn insert_matching_exit_order_for_test(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    run_id: &str,
    planned_exit_time: i64,
    status: &str,
    success: i64,
    order_id: &str,
    client_order_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, created_at
        ) VALUES (?, ?, ?, ?, 'sell', 'market', 1.0, 100.0, ?, ?, ?, 'close_position',
          '', ?, ?, ?, ?, '2026-01-01T00:00:30+00:00')
        "#,
    )
    .bind(&config.strategy_id)
    .bind(&config.strategy_name)
    .bind(&config.symbol)
    .bind(&config.symbol)
    .bind(order_id)
    .bind(client_order_id)
    .bind(status)
    .bind(&config.mode)
    .bind(success)
    .bind(run_id)
    .bind(planned_exit_time)
    .execute(pool)
    .await
    .expect("test exit order should insert");
}

fn temp_db_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
        .join("market.db")
}
