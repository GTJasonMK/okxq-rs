use std::collections::BTreeSet;

use serde_json::json;
use sqlx::SqlitePool;
use tauri::State;

use crate::{
    app_state::AppState,
    error::AppResult,
    storage,
    sync_record_summary::{
        load_all_sync_record_market_aggregates, normalize_sync_record_inst_type,
        watched_market_scopes, MarketScope,
    },
};

use super::formatting::{format_bytes, format_uptime};

#[tauri::command]
pub async fn system_status(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    system_status_value(&state).await
}

pub async fn system_status_value(state: &AppState) -> AppResult<serde_json::Value> {
    let uptime_seconds = (chrono::Utc::now() - state.started_at).num_seconds().max(0);
    let (db_path, api_configured, demo_configured, live_configured, mode, rate_limit) = {
        let cfg = state.config.read().await;
        (
            cfg.database_path
                .clone()
                .unwrap_or_else(|| state.paths.data_dir.join("market.db")),
            cfg.okx.is_valid(),
            cfg.okx.demo.is_valid(),
            cfg.okx.live.is_valid(),
            cfg.okx.default_mode(),
            cfg.cache.okx_rate_limit,
        )
    };
    let db_size = storage::db_size_bytes(&db_path).await;
    let watched = state.preferences.watched_symbols().await?;
    let managed_scopes = watched_market_scopes(&watched);
    let managed_market_count = managed_scopes.len();
    let managed_symbol_count = watched
        .iter()
        .filter(|item| item.sync_spot || item.sync_swap)
        .count();
    let market_summary = market_data_summary_from_sync_records(&state.db, &managed_scopes).await;

    Ok(json!({
        "system": {
            "uptime": format_uptime(uptime_seconds),
            "uptime_seconds": uptime_seconds,
            "rust_version": option_env!("RUSTC_VERSION").unwrap_or("unknown"),
            "os": std::env::consts::OS,
            "pid": std::process::id(),
            "memory_mb": null,
            "cpu_percent": null
        },
        "cache": {
            "candle_entries": 0,
            "sync_cooldowns": 0,
            "ticker_entries": 0
        },
        "data": {
            "symbol_count": managed_symbol_count,
            "market_count": managed_market_count,
            "candle_count": market_summary.managed_candle_count,
            "db_symbol_count": market_summary.db_symbol_count,
            "db_market_count": market_summary.db_market_count,
            "db_candle_count": market_summary.db_candle_count,
            "db_size": format_bytes(db_size),
            "database_path": db_path.display().to_string()
        },
        "paths": {
            "root": state.paths.root.display().to_string(),
            "config_dir": state.paths.config_dir.display().to_string(),
            "data_dir": state.paths.data_dir.display().to_string(),
            "logs_dir": state.paths.logs_dir.display().to_string()
        },
        "okx": {
            "api_configured": api_configured,
            "demo_configured": demo_configured,
            "live_configured": live_configured,
            "mode": mode,
            "api_accessible": false,
            "data_timestamp": null
        },
        "rate_limit": {
            "total_calls": 0,
            "calls_per_minute": 0,
            "rate_limit": rate_limit,
            "remaining_quota": rate_limit,
            "usage_percent": 0,
            "mode": "rust_governor_pending"
        }
    }))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MarketDataSummary {
    managed_candle_count: i64,
    db_symbol_count: usize,
    db_market_count: usize,
    db_candle_count: i64,
}

async fn market_data_summary_from_sync_records(
    pool: &SqlitePool,
    managed_scopes: &BTreeSet<MarketScope>,
) -> MarketDataSummary {
    query_market_data_summary_from_sync_records(pool, managed_scopes)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "failed to summarize sync_records for system status");
            MarketDataSummary::default()
        })
}

async fn query_market_data_summary_from_sync_records(
    pool: &SqlitePool,
    managed_scopes: &BTreeSet<MarketScope>,
) -> AppResult<MarketDataSummary> {
    let records = load_all_sync_record_market_aggregates(pool)
        .await?
        .into_iter()
        .map(|record| (record.inst_id, record.inst_type, record.candle_count));
    Ok(summarize_market_records(records, managed_scopes))
}

fn summarize_market_records<I>(
    records: I,
    managed_scopes: &BTreeSet<MarketScope>,
) -> MarketDataSummary
where
    I: IntoIterator<Item = (String, String, i64)>,
{
    let mut summary = MarketDataSummary::default();
    let mut db_symbols = BTreeSet::<String>::new();
    let mut db_markets = BTreeSet::<(String, String)>::new();
    for (raw_inst_id, raw_inst_type, raw_candle_count) in records {
        let inst_id = raw_inst_id.trim().to_uppercase();
        if inst_id.is_empty() {
            continue;
        }
        let Some(inst_type) = normalize_sync_record_inst_type(&raw_inst_type) else {
            continue;
        };
        let candle_count = raw_candle_count.max(0);
        db_symbols.insert(inst_id.clone());
        db_markets.insert((inst_id.clone(), inst_type.clone()));
        summary.db_candle_count += candle_count;
        if managed_scopes.contains(&(inst_id, inst_type)) {
            summary.managed_candle_count += candle_count;
        }
    }
    summary.db_symbol_count = db_symbols.len();
    summary.db_market_count = db_markets.len();
    summary
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[test]
    fn summarize_market_records_uses_sync_record_counts_without_candle_scan() {
        let managed_scopes = BTreeSet::from([
            ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
            ("ETH-USDT".to_string(), "SPOT".to_string()),
        ]);

        let summary = summarize_market_records(
            [
                ("BTC-USDT-SWAP".to_string(), "SWAP".to_string(), 100),
                ("BTC-USDT-SWAP".to_string(), "SWAP".to_string(), 25),
                ("ETH-USDT".to_string(), "SPOT".to_string(), 50),
                ("SOL-USDT-SWAP".to_string(), "SWAP".to_string(), 30),
                ("BAD".to_string(), "SPOT".to_string(), -10),
            ],
            &managed_scopes,
        );

        assert_eq!(summary.managed_candle_count, 175);
        assert_eq!(summary.db_candle_count, 205);
        assert_eq!(summary.db_symbol_count, 4);
        assert_eq!(summary.db_market_count, 4);
    }

    #[tokio::test]
    async fn sql_market_data_summary_aggregates_sync_records_by_market() {
        let pool = memory_pool().await;
        create_sync_records_table(&pool).await;
        insert_sync_record_rows(
            &pool,
            &[
                ("BTC-USDT-SWAP", "SWAP", "1m", 100),
                ("BTC-USDT-SWAP", "SWAP", "5m", 25),
                ("ETH-USDT", "SPOT", "1m", 50),
                ("SOL-USDT-SWAP", "SWAP", "1m", 30),
                ("BAD", "SPOT", "1m", -10),
                ("   ", "SPOT", "1m", 999),
            ],
        )
        .await;
        let managed_scopes = BTreeSet::from([
            ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
            ("ETH-USDT".to_string(), "SPOT".to_string()),
        ]);

        let summary = query_market_data_summary_from_sync_records(&pool, &managed_scopes)
            .await
            .expect("market data summary");

        assert_eq!(
            summary,
            MarketDataSummary {
                managed_candle_count: 175,
                db_candle_count: 205,
                db_symbol_count: 4,
                db_market_count: 4,
            }
        );
    }

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    async fn create_sync_records_table(pool: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE sync_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT NOT NULL,
                inst_type TEXT NOT NULL DEFAULT 'SPOT',
                timeframe TEXT NOT NULL,
                candle_count INTEGER DEFAULT 0,
                UNIQUE(inst_id, inst_type, timeframe)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create sync_records table");
    }

    async fn insert_sync_record_rows(pool: &SqlitePool, rows: &[(&str, &str, &str, i64)]) {
        let mut tx = pool.begin().await.expect("begin tx");
        for (inst_id, inst_type, timeframe, candle_count) in rows {
            sqlx::query(
                "INSERT INTO sync_records (inst_id, inst_type, timeframe, candle_count) VALUES (?, ?, ?, ?)",
            )
            .bind(inst_id)
            .bind(inst_type)
            .bind(timeframe)
            .bind(candle_count)
            .execute(&mut *tx)
            .await
            .expect("insert sync record");
        }
        tx.commit().await.expect("commit tx");
    }
}
