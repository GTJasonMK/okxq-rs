use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::{
    error::{AppError, AppResult},
    live_strategy::types::LiveExecutionLogEntry,
};

const MAX_LOG_QUERY_LIMIT: i64 = 300;
const RETAIN_LOGS_PER_RUN: i64 = 2_000;
const RETAIN_LOGS_GLOBAL: i64 = 50_000;

pub(in crate::live_strategy) async fn insert_live_execution_log(
    pool: &SqlitePool,
    entry: &LiveExecutionLogEntry,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO live_execution_logs (
            run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
            seq, timestamp_ms, time, stage, level, message, details_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(entry.run_id.trim())
    .bind(entry.mode.trim())
    .bind(entry.strategy_id.trim())
    .bind(entry.strategy_name.trim())
    .bind(entry.symbol.trim())
    .bind(entry.inst_type.trim())
    .bind(entry.timeframe.trim())
    .bind(entry.seq as i64)
    .bind(entry.timestamp_ms)
    .bind(entry.time.trim())
    .bind(entry.stage.trim())
    .bind(entry.level.trim())
    .bind(entry.message.as_str())
    .bind(entry.details.to_string())
    .execute(pool)
    .await?;
    prune_live_execution_logs(pool, entry.run_id.trim()).await?;
    Ok(())
}

pub async fn query_live_execution_logs(
    pool: &SqlitePool,
    limit: i64,
    mode: &str,
    run_id: &str,
) -> AppResult<Vec<LiveExecutionLogEntry>> {
    let limit = limit.clamp(1, MAX_LOG_QUERY_LIMIT);
    let mode = mode.trim();
    let run_id = run_id.trim();
    let rows = if !mode.is_empty() && !run_id.is_empty() {
        sqlx::query(
            r#"
            SELECT run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
                   seq, timestamp_ms, time, stage, level, message, details_json
            FROM live_execution_logs
            WHERE mode = ? AND run_id = ?
            ORDER BY timestamp_ms DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(mode)
        .bind(run_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else if !mode.is_empty() {
        sqlx::query(
            r#"
            SELECT run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
                   seq, timestamp_ms, time, stage, level, message, details_json
            FROM live_execution_logs
            WHERE mode = ?
            ORDER BY timestamp_ms DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(mode)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else if run_id.is_empty() {
        sqlx::query(
            r#"
            SELECT run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
                   seq, timestamp_ms, time, stage, level, message, details_json
            FROM live_execution_logs
            ORDER BY timestamp_ms DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
                   seq, timestamp_ms, time, stage, level, message, details_json
            FROM live_execution_logs
            WHERE run_id = ?
            ORDER BY timestamp_ms DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(run_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    let mut entries = rows
        .into_iter()
        .map(live_execution_log_row_to_entry)
        .collect::<AppResult<Vec<_>>>()?;
    entries.reverse();
    Ok(entries)
}

fn live_execution_log_row_to_entry(row: SqliteRow) -> AppResult<LiveExecutionLogEntry> {
    let seq = row.try_get::<i64, _>("seq")?;
    if seq < 0 {
        return Err(AppError::Runtime(
            "live execution log seq 不能为负数".to_string(),
        ));
    }
    Ok(LiveExecutionLogEntry {
        seq: seq as u64,
        run_id: row.try_get::<String, _>("run_id")?,
        mode: row.try_get::<String, _>("mode")?,
        strategy_id: row.try_get::<String, _>("strategy_id")?,
        strategy_name: row.try_get::<String, _>("strategy_name")?,
        symbol: row.try_get::<String, _>("symbol")?,
        inst_type: row.try_get::<String, _>("inst_type")?,
        timeframe: row.try_get::<String, _>("timeframe")?,
        timestamp_ms: row.try_get::<i64, _>("timestamp_ms")?,
        time: row.try_get::<String, _>("time")?,
        stage: row.try_get::<String, _>("stage")?,
        level: row.try_get::<String, _>("level")?,
        message: row.try_get::<String, _>("message")?,
        details: details_json_value(&row)?,
    })
}

fn details_json_value(row: &SqliteRow) -> AppResult<Value> {
    let raw = row.try_get::<String, _>("details_json")?;
    serde_json::from_str::<Value>(&raw).map_err(|error| {
        AppError::Runtime(format!(
            "解析 live execution log details_json 失败: {error}"
        ))
    })
}

async fn prune_live_execution_logs(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    prune_live_execution_logs_with_limits(pool, run_id, RETAIN_LOGS_PER_RUN, RETAIN_LOGS_GLOBAL)
        .await
}

async fn prune_live_execution_logs_with_limits(
    pool: &SqlitePool,
    run_id: &str,
    per_run_limit: i64,
    global_limit: i64,
) -> AppResult<()> {
    let per_run_limit = per_run_limit.max(1);
    let global_limit = global_limit.max(1);
    let run_id = run_id.trim();
    if !run_id.is_empty() {
        sqlx::query(
            r#"
            DELETE FROM live_execution_logs
            WHERE id IN (
                SELECT id
                FROM live_execution_logs
                WHERE run_id = ?
                ORDER BY timestamp_ms DESC, id DESC
                LIMIT -1 OFFSET ?
            )
            "#,
        )
        .bind(run_id)
        .bind(per_run_limit)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        r#"
        DELETE FROM live_execution_logs
        WHERE id IN (
            SELECT id
            FROM live_execution_logs
            ORDER BY timestamp_ms DESC, id DESC
            LIMIT -1 OFFSET ?
        )
        "#,
    )
    .bind(global_limit)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }

    fn log_entry(run_id: &str, seq: u64, message: &str) -> LiveExecutionLogEntry {
        LiveExecutionLogEntry {
            seq,
            run_id: run_id.to_string(),
            mode: "simulated".to_string(),
            strategy_id: "strategy-a".to_string(),
            strategy_name: "Strategy A".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "15m".to_string(),
            timestamp_ms: 1_780_000_000_000 + seq as i64,
            time: format!("2026-06-08T00:00:{seq:02}Z"),
            stage: "submit".to_string(),
            level: "info".to_string(),
            message: message.to_string(),
            details: json!({ "seq": seq }),
        }
    }

    async fn insert_raw_log_row(pool: &SqlitePool, seq: i64, details_json: &str) {
        sqlx::query(
            r#"
            INSERT INTO live_execution_logs (
                run_id, mode, strategy_id, strategy_name, symbol, inst_type, timeframe,
                seq, timestamp_ms, time, stage, level, message, details_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("run-dirty")
        .bind("simulated")
        .bind("strategy-a")
        .bind("Strategy A")
        .bind("BTC-USDT-SWAP")
        .bind("SWAP")
        .bind("15m")
        .bind(seq)
        .bind(1_780_000_000_000_i64)
        .bind("2026-06-08T00:00:00Z")
        .bind("submit")
        .bind("info")
        .bind("dirty")
        .bind(details_json)
        .execute(pool)
        .await
        .expect("raw log row should insert");
    }

    #[tokio::test]
    async fn live_execution_logs_persist_and_query_by_run() {
        let db_path = temp_db_path("live_execution_logs_persist");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        insert_live_execution_log(&pool, &log_entry("run-a", 1, "A1"))
            .await
            .expect("first log should insert");
        insert_live_execution_log(&pool, &log_entry("run-b", 2, "B1"))
            .await
            .expect("second log should insert");
        insert_live_execution_log(&pool, &log_entry("run-a", 3, "A2"))
            .await
            .expect("third log should insert");

        let rows = query_live_execution_logs(&pool, 10, "simulated", "run-a")
            .await
            .expect("logs should query");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].message, "A1");
        assert_eq!(rows[0].mode, "simulated");
        assert_eq!(rows[0].strategy_id, "strategy-a");
        assert_eq!(rows[0].symbol, "BTC-USDT-SWAP");
        assert_eq!(rows[1].message, "A2");
        assert_eq!(rows[1].details["seq"], json!(3));

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_limit_latest_then_return_chronological() {
        let db_path = temp_db_path("live_execution_logs_limit");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        insert_live_execution_log(&pool, &log_entry("run-a", 1, "A1"))
            .await
            .expect("first log should insert");
        insert_live_execution_log(&pool, &log_entry("run-a", 2, "A2"))
            .await
            .expect("second log should insert");
        insert_live_execution_log(&pool, &log_entry("run-a", 3, "A3"))
            .await
            .expect("third log should insert");

        let rows = query_live_execution_logs(&pool, 2, "simulated", "run-a")
            .await
            .expect("logs should query");

        assert_eq!(
            rows.iter()
                .map(|entry| entry.message.as_str())
                .collect::<Vec<_>>(),
            vec!["A2", "A3"]
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_reject_dirty_details_json() {
        let db_path = temp_db_path("live_execution_logs_dirty_details");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        insert_raw_log_row(&pool, 1, "{invalid").await;

        let error = query_live_execution_logs(&pool, 10, "simulated", "run-dirty")
            .await
            .expect_err("dirty details_json should fail");
        assert!(
            error.to_string().contains("details_json"),
            "unexpected error: {error}"
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_reject_negative_seq() {
        let db_path = temp_db_path("live_execution_logs_dirty_seq");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        insert_raw_log_row(&pool, -1, r#"{"seq": -1}"#).await;

        let error = query_live_execution_logs(&pool, 10, "simulated", "run-dirty")
            .await
            .expect_err("negative seq should fail");
        assert!(
            error.to_string().contains("seq"),
            "unexpected error: {error}"
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_prune_per_run_keeps_latest_rows() {
        let db_path = temp_db_path("live_execution_logs_prune_per_run");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        for seq in 1..=5 {
            insert_live_execution_log(&pool, &log_entry("run-a", seq, &format!("A{seq}")))
                .await
                .expect("log should insert");
        }
        prune_live_execution_logs_with_limits(&pool, "run-a", 3, 20)
            .await
            .expect("logs should prune");

        let rows = query_live_execution_logs(&pool, 10, "simulated", "run-a")
            .await
            .expect("logs should query");

        assert_eq!(
            rows.iter()
                .map(|entry| entry.message.as_str())
                .collect::<Vec<_>>(),
            vec!["A3", "A4", "A5"]
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_prune_global_keeps_latest_rows_across_runs() {
        let db_path = temp_db_path("live_execution_logs_prune_global");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        for seq in 1..=3 {
            insert_live_execution_log(&pool, &log_entry("run-a", seq, &format!("A{seq}")))
                .await
                .expect("run-a log should insert");
        }
        for seq in 4..=6 {
            insert_live_execution_log(&pool, &log_entry("run-b", seq, &format!("B{seq}")))
                .await
                .expect("run-b log should insert");
        }
        prune_live_execution_logs_with_limits(&pool, "run-b", 10, 4)
            .await
            .expect("logs should prune globally");

        let rows = query_live_execution_logs(&pool, 10, "", "")
            .await
            .expect("logs should query");

        assert_eq!(
            rows.iter()
                .map(|entry| entry.message.as_str())
                .collect::<Vec<_>>(),
            vec!["A3", "B4", "B5", "B6"]
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_execution_logs_filter_by_mode() {
        let db_path = temp_db_path("live_execution_logs_filter_mode");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        let simulated = log_entry("run-a", 1, "simulated");
        let mut live = log_entry("run-b", 2, "live");
        live.mode = "live".to_string();

        insert_live_execution_log(&pool, &simulated)
            .await
            .expect("simulated log should insert");
        insert_live_execution_log(&pool, &live)
            .await
            .expect("live log should insert");

        let rows = query_live_execution_logs(&pool, 10, "live", "")
            .await
            .expect("logs should query by mode");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message, "live");
        assert_eq!(rows[0].mode, "live");

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}
