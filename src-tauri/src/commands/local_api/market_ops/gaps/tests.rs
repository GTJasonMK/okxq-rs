use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use sqlx::sqlite::SqlitePoolOptions;

use super::query::{load_repair_gap_ranges, local_candles_cover_range, remaining_gap_candles};
use super::repair::{
    defer_source_sync_record_refresh, flush_deferred_source_sync_record_refreshes,
};
use super::repair_plan::{
    gap_range_value, historical_source_end_ts, plan_gap_repairs, repair_method_for_gap,
    repair_plan_targets, source_candles_for_repair, source_end_ts_for_target_range,
    source_timeframe_for_decision, GapRange, GapRangeRepairReport, RepairDecision,
};
use super::*;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create sqlite pool");
    sqlx::query(
        r#"
        CREATE TABLE candles (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          inst_id TEXT NOT NULL,
          inst_type TEXT NOT NULL DEFAULT 'SPOT',
          timeframe TEXT NOT NULL,
          timestamp INTEGER NOT NULL,
          open REAL NOT NULL,
          high REAL NOT NULL,
          low REAL NOT NULL,
          close REAL NOT NULL,
          volume REAL NOT NULL,
          volume_ccy REAL DEFAULT 0,
          created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
          UNIQUE(inst_id, inst_type, timeframe, timestamp)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create candles table");
    sqlx::query(
        "CREATE INDEX idx_candles_query ON candles(inst_id, inst_type, timeframe, timestamp)",
    )
    .execute(&pool)
    .await
    .expect("create candles index");
    sqlx::query(
        r#"
        CREATE TABLE sync_records (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          inst_id TEXT NOT NULL,
          inst_type TEXT NOT NULL DEFAULT 'SPOT',
          timeframe TEXT NOT NULL,
          last_sync_time TIMESTAMP,
          oldest_timestamp INTEGER,
          newest_timestamp INTEGER,
          candle_count INTEGER DEFAULT 0,
          history_complete INTEGER NOT NULL DEFAULT 0,
          last_sync_mode TEXT NOT NULL DEFAULT 'window',
          UNIQUE(inst_id, inst_type, timeframe)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create sync_records table");
    pool
}

async fn insert_candles(pool: &SqlitePool, timestamps: &[i64]) {
    for timestamp in timestamps {
        sqlx::query(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 1, 1, 1, 1, 1, 1)
            "#,
        )
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert candle");
    }
}

async fn insert_invalid_open_candle(pool: &SqlitePool, timestamp: i64) {
    sqlx::query(
        r#"
        INSERT INTO candles (
          inst_id, inst_type, timeframe, timestamp,
          open, high, low, close, volume, volume_ccy
        ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 0, 1, 1, 1, 1, 1)
        "#,
    )
    .bind(timestamp)
    .execute(pool)
    .await
    .expect("insert invalid candle");
}

fn request(start_ts: i64, end_ts: i64, limit: i64) -> GapPlanRequest {
    GapPlanRequest {
        inst_id: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        timeframe: "1m".to_string(),
        start_ts,
        end_ts,
        limit,
    }
}

fn local_request(body: Value) -> LocalApiRequest {
    LocalApiRequest {
        method: "POST".to_string(),
        path: "/api/market/gaps/repair/jobs".to_string(),
        params: Map::new(),
        body,
    }
}

#[tokio::test]
async fn gap_plan_reports_edge_and_internal_ranges() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 300_000]).await;

    let plan = build_gap_plan(&pool, &request(0, 420_000, 10))
        .await
        .expect("build gap plan");
    let gaps = plan.get("gaps").and_then(Value::as_array).expect("gaps");

    assert_eq!(plan["expected_candles"], json!(8));
    assert_eq!(plan["available_candles"], json!(3));
    assert_eq!(plan["missing_candles"], json!(5));
    assert_eq!(plan["gap_event_count"], json!(3));
    assert_eq!(gaps.len(), 3);
    assert_eq!(gaps[0]["start_ts"], json!(0));
    assert_eq!(gaps[0]["end_ts"], json!(0));
    assert_eq!(gaps[1]["start_ts"], json!(180_000));
    assert_eq!(gaps[1]["end_ts"], json!(240_000));
    assert_eq!(gaps[2]["start_ts"], json!(360_000));
    assert_eq!(gaps[2]["end_ts"], json!(420_000));
}

#[tokio::test]
async fn empty_series_reports_whole_requested_range_as_gap() {
    let pool = test_pool().await;

    let plan = build_gap_plan(&pool, &request(0, 180_000, 10))
        .await
        .expect("build gap plan");
    let gaps = plan.get("gaps").and_then(Value::as_array).expect("gaps");

    assert_eq!(plan["expected_candles"], json!(4));
    assert_eq!(plan["available_candles"], json!(0));
    assert_eq!(plan["missing_candles"], json!(4));
    assert_eq!(plan["gap_event_count"], json!(1));
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["start_ts"], json!(0));
    assert_eq!(gaps[0]["end_ts"], json!(180_000));
}

#[tokio::test]
async fn gap_plan_treats_invalid_market_rows_as_missing() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 180_000]).await;
    insert_invalid_open_candle(&pool, 120_000).await;

    let plan = build_gap_plan(&pool, &request(60_000, 180_000, 10))
        .await
        .expect("build gap plan");
    let gaps = plan.get("gaps").and_then(Value::as_array).expect("gaps");

    assert_eq!(plan["expected_candles"], json!(3));
    assert_eq!(plan["available_candles"], json!(2));
    assert_eq!(plan["missing_candles"], json!(1));
    assert_eq!(plan["gap_event_count"], json!(1));
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["start_ts"], json!(120_000));
    assert_eq!(gaps[0]["end_ts"], json!(120_000));
}

#[tokio::test]
async fn remaining_gap_candles_rechecks_edges_and_internal_gaps() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 300_000]).await;

    let remaining = remaining_gap_candles(&pool, &request(0, 420_000, 10), 60_000)
        .await
        .expect("remaining gap count");

    assert_eq!(remaining, 5);
}

#[tokio::test]
async fn gap_plan_limit_zero_keeps_summary_while_omitting_internal_ranges() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 300_000]).await;

    let plan = build_gap_plan(&pool, &request(60_000, 300_000, 0))
        .await
        .expect("build gap plan");
    let gaps = plan.get("gaps").and_then(Value::as_array).expect("gaps");

    assert_eq!(plan["missing_candles"], json!(2));
    assert_eq!(plan["gap_event_count"], json!(1));
    assert_eq!(plan["returned_gap_count"], json!(0));
    assert_eq!(plan["truncated"], json!(true));
    assert!(gaps.is_empty());
}

#[tokio::test]
async fn gap_repair_request_rejects_ranges_without_missing_candles() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 180_000]).await;

    let err = gap_repair_sync_job_request(&pool, &request(60_000, 180_000, 10), "auto")
        .await
        .expect_err("complete range should not create repair job");

    assert!(err.to_string().contains("没有需要补齐的缺口"));
}

#[tokio::test]
async fn preloaded_gap_ranges_build_same_repair_request_as_regular_path() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 300_000]).await;
    let plan_request = request(0, 420_000, 10);
    let timeframe_ms = timeframe_to_ms(&plan_request.timeframe).max(1);
    let ranges = load_repair_gap_ranges(&pool, &plan_request, timeframe_ms)
        .await
        .expect("load repair ranges");
    let from_ranges = gap_repair_sync_job_request_from_ranges(&plan_request, "auto", &ranges)
        .expect("build request from preloaded ranges");
    let regular = gap_repair_sync_job_request(&pool, &plan_request, "auto")
        .await
        .expect("build regular request");

    assert_eq!(
        serde_json::to_value(from_ranges).expect("request json"),
        serde_json::to_value(regular).expect("regular request json"),
    );
}

#[test]
fn deferred_source_sync_record_refresh_collapses_ranges_by_source_scope() {
    let request = GapPlanRequest {
        inst_id: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        timeframe: "1H".to_string(),
        start_ts: 0,
        end_ts: 3_600_000,
        limit: 100,
    };
    let mut deferred = BTreeMap::new();
    for index in 0..5 {
        defer_source_sync_record_refresh(
            &mut deferred,
            &request,
            &GapRangeRepairReport {
                method: "paginated".to_string(),
                start_ts: index * 60_000,
                end_ts: index * 60_000,
                missing_candles: 1,
                fetch_timeframe: BASE_CANDLE_TIMEFRAME.to_string(),
                fetched_count: 60,
                ..Default::default()
            },
        );
    }
    defer_source_sync_record_refresh(
        &mut deferred,
        &request,
        &GapRangeRepairReport {
            method: "paginated".to_string(),
            fetch_timeframe: request.timeframe.clone(),
            fetched_count: 1,
            ..Default::default()
        },
    );

    assert_eq!(deferred.len(), 1);
    assert_eq!(
        deferred
            .get(&(
                request.inst_id.clone(),
                request.inst_type.clone(),
                BASE_CANDLE_TIMEFRAME.to_string(),
            ))
            .map(String::as_str),
        Some("gap_paginated")
    );
}

#[tokio::test]
async fn flush_deferred_source_sync_record_refreshes_updates_source_sync_record() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 180_000]).await;
    let request = GapPlanRequest {
        inst_id: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        timeframe: "1H".to_string(),
        start_ts: 60_000,
        end_ts: 180_000,
        limit: 1_000,
    };
    let mut deferred = BTreeMap::new();
    defer_source_sync_record_refresh(
        &mut deferred,
        &request,
        &GapRangeRepairReport {
            method: "paginated".to_string(),
            start_ts: 60_000,
            end_ts: 60_000,
            missing_candles: 1,
            fetch_timeframe: BASE_CANDLE_TIMEFRAME.to_string(),
            fetched_count: 60,
            ..Default::default()
        },
    );
    assert_eq!(deferred.len(), 1);

    flush_deferred_source_sync_record_refreshes(&pool, deferred)
        .await
        .expect("flush deferred source refreshes");

    let record = get_sync_record_stats(&pool, "BTC-USDT-SWAP", "SWAP", BASE_CANDLE_TIMEFRAME)
        .await
        .expect("source sync record query")
        .expect("source sync record");
    assert_eq!(record.oldest_timestamp, Some(60_000));
    assert_eq!(record.newest_timestamp, Some(180_000));
    assert_eq!(record.candle_count, 3);
    assert_eq!(record.last_sync_mode, "gap_paginated");
}

#[tokio::test]
async fn local_candle_coverage_requires_complete_base_range() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 180_000]).await;

    assert!(
        local_candles_cover_range(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 60_000, 180_000)
            .await
            .expect("covered range")
    );
    assert!(
        !local_candles_cover_range(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 60_000, 240_000)
            .await
            .expect("range with missing candle")
    );
}

#[tokio::test]
async fn local_candle_coverage_requires_valid_market_rows() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 180_000]).await;
    insert_invalid_open_candle(&pool, 120_000).await;

    assert!(
        !local_candles_cover_range(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 60_000, 180_000)
            .await
            .expect("range with invalid candle")
    );
}

#[tokio::test]
async fn complete_gap_plan_skips_internal_ranges_without_losing_summary() {
    let pool = test_pool().await;
    insert_candles(&pool, &[60_000, 120_000, 180_000]).await;

    let plan = build_gap_plan(&pool, &request(60_000, 180_000, 10))
        .await
        .expect("build complete gap plan");

    assert_eq!(plan["expected_candles"], json!(3));
    assert_eq!(plan["available_candles"], json!(3));
    assert_eq!(plan["missing_candles"], json!(0));
    assert_eq!(plan["gap_event_count"], json!(0));
    assert_eq!(plan["returned_gap_count"], json!(0));
    assert_eq!(plan["gaps"].as_array().expect("gaps").len(), 0);
}

#[test]
fn gap_repair_request_requires_explicit_time_range() {
    let req = local_request(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "1m"
    }));

    let err = gap_repair_plan_request(&req).expect_err("repair job must not use default range");

    assert!(err.to_string().contains("start_ts 和 end_ts"));
}

#[test]
fn large_old_gap_prefers_historical_zip() {
    let range = GapRange {
        start_ts: 0,
        end_ts: 8 * DAY_MS,
        missing_candles: 8 * 24 * 60,
    };

    let decision = repair_method_for_gap(&range, "1m", timeframe_to_ms("1m"), 30 * DAY_MS);

    assert_eq!(decision.method, "historical_zip");
}

#[test]
fn recent_gap_prefers_paginated_even_when_large() {
    let now_ms = 30 * DAY_MS;
    let range = GapRange {
        start_ts: now_ms - 8 * DAY_MS,
        end_ts: now_ms - DAY_MS,
        missing_candles: 8 * 24 * 60,
    };

    let decision = repair_method_for_gap(&range, "1m", timeframe_to_ms("1m"), now_ms);

    assert_eq!(decision.method, "paginated");
}

#[test]
fn paginated_repair_for_derived_timeframe_uses_base_source() {
    let range = GapRange {
        start_ts: 1_704_067_200_000,
        end_ts: 1_704_067_200_000,
        missing_candles: 1,
    };
    let decision = RepairDecision {
        method: "paginated",
        reason: "test",
    };

    assert_eq!(
        source_timeframe_for_decision(&decision, "1H"),
        BASE_CANDLE_TIMEFRAME
    );
    assert_eq!(
        source_end_ts_for_target_range(&range, "1H"),
        1_704_070_740_000
    );
    assert_eq!(source_candles_for_repair(&range, &decision, "1H"), 60);
}

#[test]
fn gap_plan_marks_derived_paginated_ranges_as_base_fetches() {
    let range = GapRange {
        start_ts: 1_704_067_200_000,
        end_ts: 1_704_067_200_000,
        missing_candles: 1,
    };
    let value = gap_range_value(&range, "1H", timeframe_to_ms("1H"), 1_704_067_200_000);

    assert_eq!(value["method"], json!("paginated"));
    assert_eq!(value["fetch_timeframe"], json!(BASE_CANDLE_TIMEFRAME));
    assert_eq!(value["requires_derivation"], json!(true));
    assert_eq!(value["target_timeframes"], json!(["1H"]));
}

#[test]
fn historical_zip_source_range_covers_complete_target_bucket() {
    let range = GapRange {
        start_ts: 1_704_067_200_000,
        end_ts: 1_704_067_200_000,
        missing_candles: 1,
    };
    let decision = RepairDecision {
        method: "historical_zip",
        reason: "test",
    };

    assert_eq!(historical_source_end_ts(&range, "1H"), 1_704_070_740_000);
    assert_eq!(source_candles_for_repair(&range, &decision, "1H"), 60);
}

#[test]
fn historical_zip_base_repair_has_no_derive_target() {
    let range = GapRange {
        start_ts: 1_704_067_200_000,
        end_ts: 1_704_067_260_000,
        missing_candles: 2,
    };
    let planned = plan_gap_repairs(
        &[range],
        BASE_CANDLE_TIMEFRAME,
        timeframe_to_ms(BASE_CANDLE_TIMEFRAME),
        30 * DAY_MS,
        "historical_zip",
    );
    let targets = repair_plan_targets(&planned);

    assert_eq!(planned[0].decision.method, "historical_zip");
    assert_eq!(targets.source_candles, 2);
    assert_eq!(targets.derive_candles, 0);
}
