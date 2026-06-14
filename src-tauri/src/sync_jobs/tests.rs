use super::{manager::SyncJobStore, *};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, Row};

fn request(
    timeframe: &str,
    source_timeframe: &str,
    target_timeframes: Vec<&str>,
) -> SyncJobRequest {
    SyncJobRequest {
        inst_id: "btc-usdt-swap".to_string(),
        inst_type: "swap".to_string(),
        timeframe: timeframe.to_string(),
        source_timeframe: source_timeframe.to_string(),
        target_timeframes: target_timeframes
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        mode: "window".to_string(),
        days: 3,
        start_ts: None,
        end_ts: None,
        repair_method: String::new(),
        target_fetch_count: 0,
        target_save_count: 0,
        target_derive_count: 0,
        target_batches: 0,
    }
}

fn running_update(
    progress: i64,
    message: impl Into<String>,
    fetched_count: i64,
    target_fetch_count: i64,
    saved_count: i64,
    target_save_count: i64,
) -> SyncJobRunningUpdate {
    SyncJobRunningUpdate {
        progress,
        message: message.into(),
        fetched_count,
        target_fetch_count,
        saved_count,
        target_save_count,
        inserted_count: saved_count,
        ..Default::default()
    }
}

fn completion_payload(message: &str) -> Value {
    json!({
        "message": message,
        "fetched_count": 0,
        "target_fetch_count": 0,
        "saved_count": 0,
        "target_save_count": 0,
        "inserted_count": 0,
        "derived_count": 0,
        "target_derive_count": 0,
        "batches": 0,
        "target_batches": 0,
        "api_calls": 0,
        "candle_count": 0,
        "source_timeframe": "1m",
        "target_timeframes": ["1m", "1H"],
        "history_complete": false,
        "last_sync_mode": "window",
        "last_sync_time": null,
        "oldest_timestamp": null,
        "newest_timestamp": null,
        "oldest_time": null,
        "newest_time": null,
        "truncated": false
    })
}

async fn test_manager() -> SyncJobManager {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create in-memory sqlite pool");
    sqlx::query(
        r#"
        CREATE TABLE sync_jobs (
          task_id TEXT PRIMARY KEY,
          inst_id TEXT NOT NULL,
          inst_type TEXT NOT NULL DEFAULT 'SPOT',
          timeframe TEXT NOT NULL,
          source_timeframe TEXT NOT NULL DEFAULT '1m',
          target_timeframes TEXT NOT NULL DEFAULT '[]',
          mode TEXT NOT NULL DEFAULT 'window',
          days INTEGER NOT NULL DEFAULT 30,
          start_ts INTEGER,
          end_ts INTEGER,
          repair_method TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL DEFAULT 'queued',
          progress INTEGER NOT NULL DEFAULT 0,
          message TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL,
          started_at TEXT,
          updated_at TEXT NOT NULL,
          finished_at TEXT,
          error TEXT NOT NULL DEFAULT '',
          fetched_count INTEGER NOT NULL DEFAULT 0,
          target_fetch_count INTEGER NOT NULL DEFAULT 0,
          saved_count INTEGER NOT NULL DEFAULT 0,
          target_save_count INTEGER NOT NULL DEFAULT 0,
          inserted_count INTEGER NOT NULL DEFAULT 0,
          derived_count INTEGER NOT NULL DEFAULT 0,
          target_derive_count INTEGER NOT NULL DEFAULT 0,
          batches INTEGER NOT NULL DEFAULT 0,
          target_batches INTEGER NOT NULL DEFAULT 0,
          api_calls INTEGER NOT NULL DEFAULT 0,
          candle_count INTEGER NOT NULL DEFAULT 0,
          history_complete INTEGER NOT NULL DEFAULT 0,
          last_sync_mode TEXT NOT NULL DEFAULT 'window',
          last_sync_time TEXT,
          oldest_timestamp INTEGER,
          newest_timestamp INTEGER,
          oldest_time TEXT,
          newest_time TEXT,
          reused_existing INTEGER NOT NULL DEFAULT 0,
          truncated INTEGER NOT NULL DEFAULT 0,
          cancel_requested INTEGER NOT NULL DEFAULT 0
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create sync_jobs table");
    sqlx::query("CREATE INDEX idx_sync_jobs_recent ON sync_jobs(updated_at DESC, created_at DESC)")
        .execute(&pool)
        .await
        .expect("create sync_jobs recent index");
    SyncJobManager::new(pool)
}

#[test]
fn sync_job_key_normalizes_timeframes_and_scope() {
    let key = request("1h", "1h", vec!["4h", "1h"])
        .key()
        .expect("valid sync job key");

    assert_eq!(
        key,
        r#"BTC-USDT-SWAP:SWAP:1H:["1H","4H"]:window:3:null:null:"#
    );
}

#[test]
fn sync_job_key_distinguishes_absent_range_from_epoch_zero_range() {
    let without_range = request("1h", "1h", vec!["1h"]);
    let mut epoch_zero_range = without_range.clone();
    epoch_zero_range.start_ts = Some(0);
    epoch_zero_range.end_ts = Some(0);

    assert_ne!(
        without_range.key().expect("without range key"),
        epoch_zero_range.key().expect("epoch range key")
    );
}

#[test]
fn sync_job_key_rejects_invalid_timeframes() {
    assert!(request("", "1m", vec!["1m"]).key().is_err());
    assert!(request("1h", "bad", vec!["1m"]).key().is_err());
    assert!(request("1h", "1m", vec!["bad"]).key().is_err());
}

#[test]
fn target_timeframes_use_okx_style_order_and_case() {
    assert_eq!(
        normalize_target_timeframes(
            vec![
                "1d".to_string(),
                "1m".to_string(),
                "1h".to_string(),
                "1H".to_string()
            ],
            "4h",
        )
        .expect("valid target timeframes"),
        vec!["1m".to_string(), "1H".to_string(), "1D".to_string()]
    );
    assert_eq!(
        normalize_target_timeframes(Vec::new(), "4h").expect("default target timeframe"),
        vec!["4H".to_string()]
    );
}

#[tokio::test]
async fn active_sync_request_is_reused_until_terminal() {
    let manager = test_manager().await;
    let sync_request = request("1h", "1m", vec!["1m", "1h"]);
    let (first, reused) = manager
        .create_or_reuse(sync_request.clone())
        .await
        .expect("create sync job");
    assert!(!reused);

    let (second, reused) = manager
        .create_or_reuse(sync_request.clone())
        .await
        .expect("reuse active sync job");
    assert!(reused);
    assert_eq!(second.task_id, first.task_id);
    assert!(second.reused_existing);

    manager
        .complete_with_payload(&first.task_id, &completion_payload("完成"))
        .await
        .expect("complete sync job");
    let (third, reused) = manager
        .create_or_reuse(sync_request)
        .await
        .expect("create new sync job after terminal");
    assert!(!reused);
    assert_ne!(third.task_id, first.task_id);
}

#[tokio::test]
async fn active_sync_request_does_not_reuse_absent_range_for_epoch_zero_range() {
    let manager = test_manager().await;
    let without_range = request("1h", "1m", vec!["1m", "1h"]);
    let (first, reused) = manager
        .create_or_reuse(without_range)
        .await
        .expect("create sync job without explicit range");
    assert!(!reused);

    let mut epoch_zero_range = request("1h", "1m", vec!["1m", "1h"]);
    epoch_zero_range.start_ts = Some(0);
    epoch_zero_range.end_ts = Some(0);
    let (second, reused) = manager
        .create_or_reuse(epoch_zero_range)
        .await
        .expect("create sync job for epoch zero range");

    assert!(!reused);
    assert_ne!(second.task_id, first.task_id);
}

#[tokio::test]
async fn cancel_state_reports_cancelled_jobs() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);

    let cancelled = manager
        .cancel_job(&job.task_id, "用户取消")
        .await
        .expect("cancel sync job")
        .expect("cancelled job");

    assert_eq!(cancelled.status, "cancelled");
    assert!(manager
        .is_cancel_requested(&job.task_id)
        .await
        .expect("cancel state"));
}

#[tokio::test]
async fn cancel_state_rejects_missing_internal_job() {
    let manager = test_manager().await;

    let result = manager.is_cancel_requested("missing-sync-job").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn persisted_active_job_lookup_distinguishes_absent_range_from_epoch_zero_range() {
    let manager = test_manager().await;
    let without_range = request("1h", "1m", vec!["1m", "1h"]);
    let (persisted, reused) = manager
        .create_or_reuse(without_range)
        .await
        .expect("create persisted sync job without explicit range");
    assert!(!reused);

    let restarted = SyncJobManager::new(manager.db.clone());
    let mut epoch_zero_range = request("1h", "1m", vec!["1m", "1h"]);
    epoch_zero_range.start_ts = Some(0);
    epoch_zero_range.end_ts = Some(0);

    let found = restarted
        .fetch_active_job_for_request(&epoch_zero_range)
        .await
        .expect("fetch persisted active job");

    assert!(found.is_none(), "must not reuse {}", persisted.task_id);
}

#[tokio::test]
async fn active_superset_target_job_covers_subset_request() {
    let manager = test_manager().await;
    let (first, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create broad sync job");
    assert!(!reused);

    let (second, reused) = manager
        .create_or_reuse(request("1m", "1m", vec!["1m"]))
        .await
        .expect("reuse broad sync job");
    assert!(reused);
    assert_eq!(second.task_id, first.task_id);
}

#[tokio::test]
async fn active_list_deduplicates_covering_job_aliases_and_clears_terminal_aliases() {
    let manager = test_manager().await;
    let (first, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create broad sync job");
    assert!(!reused);

    let (covered, reused) = manager
        .create_or_reuse(request("1m", "1m", vec!["1m"]))
        .await
        .expect("reuse broad sync job");
    assert!(reused);
    assert_eq!(covered.task_id, first.task_id);

    let active = manager
        .list(true, 200, None)
        .await
        .expect("list active jobs");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].task_id, first.task_id);

    manager
        .complete_with_payload(&first.task_id, &completion_payload("完成"))
        .await
        .expect("complete sync job");
    let active = manager
        .list(true, 200, None)
        .await
        .expect("list active jobs after completion");
    assert!(active.is_empty());
}

#[test]
fn active_coverage_index_reuses_covering_job_and_prunes_stale_ids() {
    let mut store = SyncJobStore::default();
    let request = coverage_subset_request(7);
    let stale = coverage_superset_job(7, "sync_a_stale");
    let covering = coverage_superset_job(7, "sync_z_covering");
    store.track_active_job(&stale).expect("track stale job");
    store
        .track_active_job(&covering)
        .expect("track covering job");
    store
        .jobs
        .insert(covering.task_id.clone(), covering.clone());

    let coverage_key = request.coverage_key().expect("coverage key");
    let reused = store
        .covering_active_job(&request, &coverage_key)
        .expect("indexed covering lookup")
        .expect("covering job should be reused");

    assert_eq!(reused.task_id, "sync_z_covering");
    assert!(!store
        .active_coverage_keys
        .get(&coverage_key)
        .into_iter()
        .flatten()
        .any(|task_id| task_id == "sync_a_stale"));
}

#[tokio::test]
async fn shorter_active_job_does_not_cover_longer_request() {
    let manager = test_manager().await;
    let (first, reused) = manager
        .create_or_reuse(request("1m", "1m", vec!["1m"]))
        .await
        .expect("create short sync job");
    assert!(!reused);

    let mut longer_request = request("1m", "1m", vec!["1m"]);
    longer_request.days = 5;
    let (second, reused) = manager
        .create_or_reuse(longer_request)
        .await
        .expect("create longer sync job");
    assert!(!reused);
    assert_ne!(second.task_id, first.task_id);
}

#[tokio::test]
async fn wait_for_terminal_returns_completed_job() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);

    let manager_clone = manager.clone();
    let task_id = job.task_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        manager_clone
            .complete_with_payload(&task_id, &completion_payload("完成"))
            .await
            .expect("complete sync job");
    });

    let completed = manager
        .wait_for_terminal(&job.task_id)
        .await
        .expect("wait for completed sync job");
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.message, "完成");
}

#[tokio::test]
async fn failed_job_keeps_last_running_progress() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(SyncJobRequest {
            target_fetch_count: 1_000,
            target_save_count: 1_000,
            target_derive_count: 100,
            target_batches: 4,
            ..request("1h", "1m", vec!["1m", "1h"])
        })
        .await
        .expect("create sync job");
    assert!(!reused);

    manager
        .mark_running(&job.task_id)
        .await
        .expect("mark running");
    manager
        .update_running(
            &job.task_id,
            SyncJobRunningUpdate {
                derived_count: 0,
                target_derive_count: 100,
                batches: 2,
                target_batches: 4,
                api_calls: 2,
                ..running_update(37, "拉取中", 300, 1_000, 240, 1_000)
            },
        )
        .await
        .expect("update progress");
    manager
        .fail(&job.task_id, "runtime error".to_string())
        .await
        .expect("mark failed");

    let failed = manager
        .get(&job.task_id)
        .await
        .expect("load failed job")
        .expect("failed job exists");
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.progress, 37);
    assert_eq!(failed.error, "runtime error");
    assert_eq!(failed.fetched_count, 300);
    assert_eq!(failed.saved_count, 240);
}

#[tokio::test]
async fn running_progress_flushes_latest_cached_values_on_terminal_failure() {
    let updates = 10;
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);

    manager
        .mark_running(&job.task_id)
        .await
        .expect("mark running");
    for index in 0..updates {
        manager
            .update_running(
                &job.task_id,
                SyncJobRunningUpdate {
                    derived_count: index as i64 / 2,
                    target_derive_count: updates as i64 / 2,
                    batches: index as i64,
                    target_batches: updates as i64,
                    api_calls: index as i64,
                    ..running_update(
                        (index % 99) as i64,
                        format!("批次 {index}"),
                        index as i64,
                        updates as i64,
                        index as i64,
                        updates as i64,
                    )
                },
            )
            .await
            .expect("running update");
    }

    let active = manager
        .get(&job.task_id)
        .await
        .expect("load active job")
        .expect("active job exists");
    assert_eq!(active.fetched_count, (updates - 1) as i64);
    assert_eq!(active.saved_count, (updates - 1) as i64);
    assert_eq!(active.message, format!("批次 {}", updates - 1));

    manager
        .fail(&job.task_id, "runtime error".to_string())
        .await
        .expect("mark failed");
    let failed = manager
        .fetch_job_by_id(&job.task_id)
        .await
        .expect("load failed job")
        .expect("failed job exists");
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.fetched_count, (updates - 1) as i64);
    assert_eq!(failed.saved_count, (updates - 1) as i64);
    assert_eq!(failed.error, "runtime error");
}

#[tokio::test]
async fn recover_interrupted_jobs_overwrites_dirty_active_finished_at() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);
    manager
        .mark_running(&job.task_id)
        .await
        .expect("mark running");
    sqlx::query("UPDATE sync_jobs SET finished_at = ? WHERE task_id = ?")
        .bind("stale-finished-at")
        .bind(&job.task_id)
        .execute(&manager.db)
        .await
        .expect("dirty active finished_at");

    manager
        .recover_interrupted_jobs()
        .await
        .expect("recover interrupted jobs");

    let recovered = sqlx::query("SELECT status, finished_at FROM sync_jobs WHERE task_id = ?")
        .bind(&job.task_id)
        .fetch_one(&manager.db)
        .await
        .expect("recovered job row");
    assert_eq!(recovered.try_get::<String, _>("status").unwrap(), "failed");
    assert_ne!(
        recovered
            .try_get::<Option<String>, _>("finished_at")
            .unwrap()
            .as_deref(),
        Some("stale-finished-at")
    );
}

#[tokio::test]
async fn completion_payload_uses_typed_json_contract() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);

    manager
        .complete_with_payload(
            &job.task_id,
            &json!({
                "message": "完成",
                "fetched_count": 300,
                "target_fetch_count": 360,
                "saved_count": 240,
                "target_save_count": 360,
                "inserted_count": 220,
                "derived_count": 120,
                "target_derive_count": 180,
                "batches": 3,
                "target_batches": 4,
                "api_calls": 5,
                "candle_count": 240,
                "source_timeframe": "1m",
                "target_timeframes": ["1m", "1H"],
                "history_complete": true,
                "last_sync_mode": "window",
                "last_sync_time": "2026-05-28T00:00:00.000Z",
                "oldest_timestamp": 1_700_000_000_000_i64,
                "newest_timestamp": 1_700_003_600_000_i64,
                "oldest_time": "2023-11-14T22:13:20.000Z",
                "newest_time": "2023-11-14T23:13:20.000Z",
                "truncated": true
            }),
        )
        .await
        .expect("complete job");

    let completed = manager
        .get(&job.task_id)
        .await
        .expect("load completed job")
        .expect("completed job exists");
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.message, "完成");
    assert_eq!(completed.fetched_count, 300);
    assert_eq!(completed.target_fetch_count, 360);
    assert_eq!(completed.saved_count, 240);
    assert_eq!(completed.inserted_count, 220);
    assert_eq!(completed.derived_count, 120);
    assert_eq!(completed.api_calls, 5);
    assert_eq!(completed.candle_count, 240);
    assert_eq!(
        completed.target_timeframes,
        vec!["1m".to_string(), "1H".to_string()]
    );
    assert!(completed.history_complete);
    assert_eq!(completed.last_sync_mode, "window");
    assert_eq!(completed.oldest_timestamp, Some(1_700_000_000_000));
    assert_eq!(completed.newest_timestamp, Some(1_700_003_600_000));
    assert!(completed.truncated);
}

#[tokio::test]
async fn completion_payload_rejects_missing_required_fields_without_mutating_job() {
    let manager = test_manager().await;
    let (job, reused) = manager
        .create_or_reuse(request("1h", "1m", vec!["1m", "1h"]))
        .await
        .expect("create sync job");
    assert!(!reused);

    let err = manager
        .complete_with_payload(&job.task_id, &json!({ "message": "完成" }))
        .await
        .expect_err("missing completion fields must fail");
    assert!(err.to_string().contains("source_timeframe"));

    let stored = manager
        .get(&job.task_id)
        .await
        .expect("load job after rejected completion")
        .expect("job should still exist");
    assert_eq!(stored.status, "queued");
    assert_eq!(stored.message, "等待开始");
}

#[tokio::test]
async fn list_task_ids_fetches_requested_jobs_and_applies_limit() {
    let manager = test_manager().await;
    let mut task_ids = Vec::new();
    for index in 0..3 {
        let (job, reused) = manager
            .create_or_reuse(SyncJobRequest {
                inst_id: format!("BENCH-{index:03}-USDT-SWAP"),
                ..request("1h", "1m", vec!["1m", "1h"])
            })
            .await
            .expect("create bench sync job");
        assert!(!reused);
        task_ids.push(job.task_id);
    }

    task_ids.push("missing-task".to_string());
    let jobs = manager
        .list(false, 10, Some(&task_ids))
        .await
        .expect("list task ids");
    let returned_ids = jobs
        .iter()
        .map(|job| job.task_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_ids = task_ids[..3]
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(returned_ids, expected_ids);

    let limited = manager
        .list(false, 2, Some(&task_ids))
        .await
        .expect("list task ids with limit");
    assert_eq!(limited.len(), 2);
}

#[tokio::test]
async fn list_task_ids_reads_cached_running_jobs_before_persisted_rows() {
    let manager = test_manager().await;
    let mut task_ids = Vec::new();
    for index in 0..3 {
        let (job, reused) = manager
            .create_or_reuse(SyncJobRequest {
                inst_id: format!("OBSERVE-{index:04}-USDT-SWAP"),
                ..request("1h", "1m", vec!["1m", "1h"])
            })
            .await
            .expect("create observed sync job");
        assert!(!reused);
        manager
            .mark_running(&job.task_id)
            .await
            .expect("mark observed job running");
        task_ids.push(job.task_id);
    }

    manager
        .update_running(
            &task_ids[1],
            SyncJobRunningUpdate {
                inserted_count: 40,
                derived_count: 7,
                target_derive_count: 10,
                batches: 4,
                target_batches: 8,
                api_calls: 3,
                ..running_update(42, "缓存中的最新进度", 42, 100, 41, 100)
            },
        )
        .await
        .expect("update cached running job");

    let jobs = manager
        .list(false, 10, Some(&task_ids))
        .await
        .expect("list observed task ids");
    let updated = jobs
        .iter()
        .find(|job| job.task_id == task_ids[1])
        .expect("updated job should be returned");

    assert_eq!(jobs.len(), 3);
    assert_eq!(updated.progress, 42);
    assert_eq!(updated.message, "缓存中的最新进度");
    assert_eq!(updated.fetched_count, 42);
    assert_eq!(updated.saved_count, 41);
}

#[tokio::test]
async fn recent_job_list_uses_updated_created_index() {
    let manager = test_manager().await;
    let plan = sqlx::query(
        "EXPLAIN QUERY PLAN SELECT task_id FROM sync_jobs ORDER BY updated_at DESC, created_at DESC LIMIT 200",
    )
    .fetch_all(&manager.db)
    .await
    .expect("explain recent sync jobs query")
    .into_iter()
    .map(|row| row.try_get::<String, _>("detail").unwrap_or_default())
    .collect::<Vec<_>>()
    .join("\n");

    assert!(
        plan.contains("idx_sync_jobs_recent"),
        "expected recent sync job query to use idx_sync_jobs_recent, got:\n{plan}"
    );
}

fn coverage_subset_request(index: usize) -> SyncJobRequest {
    SyncJobRequest {
        inst_id: format!("COVERAGE-{index:04}-USDT-SWAP"),
        inst_type: "SWAP".to_string(),
        timeframe: "1m".to_string(),
        source_timeframe: "1m".to_string(),
        target_timeframes: vec!["1m".to_string()],
        mode: "window".to_string(),
        days: 3,
        start_ts: None,
        end_ts: None,
        repair_method: String::new(),
        target_fetch_count: 0,
        target_save_count: 0,
        target_derive_count: 0,
        target_batches: 0,
    }
}

fn coverage_superset_job(index: usize, task_id: &str) -> SyncJob {
    SyncJob {
        task_id: task_id.to_string(),
        inst_id: format!("COVERAGE-{index:04}-USDT-SWAP"),
        inst_type: "SWAP".to_string(),
        timeframe: "1H".to_string(),
        source_timeframe: "1m".to_string(),
        target_timeframes: vec!["1m".to_string(), "1H".to_string()],
        mode: "window".to_string(),
        days: 30,
        start_ts: None,
        end_ts: None,
        repair_method: String::new(),
        status: "running".to_string(),
        progress: 20,
        message: "同步中".to_string(),
        created_at: "2026-05-23T08:00:00.000Z".to_string(),
        started_at: Some("2026-05-23T08:00:00.000Z".to_string()),
        updated_at: "2026-05-23T08:00:00.000Z".to_string(),
        finished_at: None,
        error: String::new(),
        fetched_count: 0,
        target_fetch_count: 0,
        saved_count: 0,
        target_save_count: 0,
        inserted_count: 0,
        derived_count: 0,
        target_derive_count: 0,
        batches: 0,
        target_batches: 0,
        api_calls: 0,
        candle_count: 0,
        history_complete: false,
        last_sync_mode: "window".to_string(),
        last_sync_time: None,
        oldest_timestamp: None,
        newest_timestamp: None,
        oldest_time: None,
        newest_time: None,
        reused_existing: false,
        truncated: false,
        cancel_requested: false,
    }
}
