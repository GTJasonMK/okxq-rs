use crate::commands::local_api::LocalApiRequest;
use crate::okx::OkxCandle;
use crate::storage;
use crate::strategy_engine::{BacktestReport, StrategyConfig};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::path::{Path, PathBuf};

use super::{
    inputs::{
        load_backtest_candles_from_db, load_backtest_context_candles_from_db,
        normalize_strategy_inst_id,
    },
    reports::backtest_report_value,
    reports::backtest_summary_row,
    runner::{
        backtest_context_prefix_len_at_or_before, backtest_context_required_end_ts,
        backtest_context_required_start_ts, backtest_execution_delay_ms,
        backtest_execution_submit_timestamp, backtest_instrument_rules_source,
        backtest_required_instruments, backtest_strict_instrument_rules,
        compact_backtest_diagnostics_enabled, compact_strategy_diagnostics,
        config_with_okx_instrument_rules_from_instruments,
        config_with_resolved_instrument_rules_source, config_with_simulated_instrument_rules,
        config_with_simulated_instrument_rules_for, context_candles_cover_window_warmup,
        default_backtest_primary_min_bars, BacktestInstrumentRulesSource,
    },
    window::{
        backtest_window, expected_backtest_candle_count, should_persist_backtest_result,
        BacktestWindow, DAY_MS, MAX_BACKTEST_CANDLES,
    },
};

mod candles;
mod reports;
mod runner;
mod window;

const HOUR_MS: i64 = 3_600_000;

fn request(body: Value) -> LocalApiRequest {
    LocalApiRequest {
        method: "POST".to_string(),
        path: "/api/backtest/run/test_runtime_strategy".to_string(),
        params: Map::new(),
        body,
    }
}

fn utc_ms(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
) -> i64 {
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_milli_opt(hour, minute, second, millis)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

fn test_strategy_config() -> StrategyConfig {
    StrategyConfig {
        strategy_id: "test_runtime_strategy".to_string(),
        strategy_name: "Test Runtime Strategy".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        timeframe: "1H".to_string(),
        initial_capital: 10_000.0,
        position_size: 0.2,
        stop_loss: 0.05,
        take_profit: 0.10,
        params: json!({
            "short_period": 3,
            "long_period": 8,
            "use_ema": false
        }),
    }
}

fn test_candles(count: i64) -> Vec<OkxCandle> {
    let base_ts = chrono::Utc::now().timestamp_millis() - count * HOUR_MS;
    (0..count)
        .map(|index| OkxCandle {
            timestamp: base_ts + index * HOUR_MS,
            open: 100.0 + index as f64,
            high: 101.0 + index as f64,
            low: 99.0 + index as f64,
            close: 100.0 + index as f64,
            volume: 1.0,
            volume_ccy: 100.0,
            volume_quote: 100.0,
            confirm: "1".to_string(),
        })
        .collect()
}

fn test_backtest_report(config: &StrategyConfig, candles: &[OkxCandle]) -> BacktestReport {
    BacktestReport {
        strategy_name: config.strategy_name.clone(),
        strategy_id: config.strategy_id.clone(),
        symbol: config.symbol.clone(),
        inst_type: config.inst_type.clone(),
        timeframe: config.timeframe.clone(),
        days: 2,
        start_time: "2026-01-01T00:00:00Z".to_string(),
        end_time: "2026-01-02T00:00:00Z".to_string(),
        initial_capital: config.initial_capital,
        final_capital: config.initial_capital,
        total_return: 0.0,
        annual_return: 0.0,
        max_drawdown: 0.0,
        sharpe_ratio: 0.0,
        sortino_ratio: 0.0,
        calmar_ratio: 0.0,
        omega_ratio: 0.0,
        win_rate: 0.0,
        profit_factor: 0.0,
        total_trades: 0,
        winning_trades: 0,
        losing_trades: 0,
        avg_profit: 0.0,
        avg_loss: 0.0,
        largest_profit: 0.0,
        largest_loss: 0.0,
        total_commission: 0.0,
        params: config.params.clone(),
        detail: json!({
            "candles": candles.iter().map(OkxCandle::to_json).collect::<Vec<_>>(),
            "equity_curve": [],
            "equity_curve_sampled": false,
            "equity_points_total": 0,
            "trades": [],
            "trade_cashflows": [],
            "trade_cashflows_total": 0,
            "trade_events_total": 0,
            "trades_truncated": false,
            "indicators": {},
            "strategy_actions": [],
            "strategy_diagnostics": {}
        }),
        sample_step: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

async fn insert_candles(
    pool: &sqlx::SqlitePool,
    config: &StrategyConfig,
    base_ts: i64,
    count: i64,
) {
    for index in 0..count {
        let close = 100.0 + index as f64;
        sqlx::query(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&config.symbol)
        .bind(&config.inst_type)
        .bind(&config.timeframe)
        .bind(base_ts + index * HOUR_MS)
        .bind(close - 0.5)
        .bind(close + 1.0)
        .bind(close - 1.0)
        .bind(close)
        .bind(1.0)
        .bind(100.0)
        .bind(100.0)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn insert_invalid_close_candle(
    pool: &sqlx::SqlitePool,
    config: &StrategyConfig,
    timestamp: i64,
) {
    sqlx::query(
        r#"
        INSERT INTO candles (
          inst_id, inst_type, timeframe, timestamp,
          open, high, low, close, volume, volume_ccy, volume_quote
        ) VALUES (?, ?, ?, ?, 100, 101, 99, 'bad-close', 1, 100, 100)
        "#,
    )
    .bind(&config.symbol)
    .bind(&config.inst_type)
    .bind(&config.timeframe)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

async fn count_backtest_results(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query("SELECT COUNT(*) AS count FROM backtest_results")
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get::<i64, _>("count")
        .unwrap()
}

async fn query_plan_details(
    pool: &sqlx::SqlitePool,
    sql: &str,
    bindings: &[(&str, &str)],
) -> Vec<String> {
    let mut query = sqlx::query(sql);
    for (name, value) in bindings {
        query = match *name {
            "limit" => query.bind(value.parse::<i64>().unwrap()),
            _ => query.bind(*value),
        };
    }
    query
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.try_get::<String, _>("detail").unwrap())
        .collect()
}

fn test_db_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "okxq-rs-{label}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

async fn cleanup_db(pool: sqlx::SqlitePool, path: &Path) {
    pool.close().await;
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
}
