use sqlx::SqlitePool;

use crate::{market_candle_rows::load_latest_valid_okx_candles, okx::OkxCandle};

use super::*;

pub(crate) async fn resolve_agent_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
) -> AppResult<(String, String)> {
    super::super::resolve_enabled_market_scope(state, inst_id, inst_type).await
}

fn normalize_agent_inst_type(inst_id: &str, inst_type: &str) -> AppResult<String> {
    let requested = inst_type.trim().to_uppercase();
    let normalized = if requested.is_empty() {
        infer_inst_type(inst_id)
    } else {
        requested
    };
    match normalized.as_str() {
        "SPOT" | "SWAP" => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "当前仅支持 SPOT/SWAP 数据读取，收到 inst_type={normalized}"
        ))),
    }
}

pub(crate) async fn enabled_watchlist_inst_ids(
    state: &AppState,
    inst_type: &str,
    limit: Option<usize>,
) -> AppResult<Vec<String>> {
    let normalized_type = normalize_agent_inst_type("", inst_type)?;
    let mut inst_ids = Vec::new();
    for item in state.preferences.watched_symbols().await? {
        let inst_id = match normalized_type.as_str() {
            "SPOT" if item.sync_spot => item.spot_inst_id,
            "SWAP" if item.sync_swap => item.swap_inst_id,
            _ => continue,
        };
        inst_ids.push(inst_id);
        if limit.is_some_and(|limit| inst_ids.len() >= limit) {
            break;
        }
    }
    Ok(inst_ids)
}

pub(crate) async fn resolve_agent_watchlist_inst_type(
    state: &AppState,
    inst_type: &str,
) -> AppResult<String> {
    let requested = inst_type.trim();
    if !requested.is_empty() {
        let normalized = normalize_agent_inst_type("", requested)?;
        if enabled_watchlist_inst_ids(state, &normalized, Some(1))
            .await?
            .is_empty()
        {
            return Err(AppError::Validation(format!(
                "关注清单未启用 {normalized} 数据目标，已拒绝批量分析"
            )));
        }
        return Ok(normalized);
    }

    let mut types = std::collections::BTreeSet::new();
    for item in state.preferences.watched_symbols().await? {
        if item.sync_spot {
            types.insert("SPOT".to_string());
        }
        if item.sync_swap {
            types.insert("SWAP".to_string());
        }
    }

    match types.len() {
        1 => Ok(types.into_iter().next().unwrap_or_default()),
        0 => Err(AppError::Validation(
            "关注清单没有启用任何批量分析数据目标".to_string(),
        )),
        _ => Err(AppError::Validation(
            "关注清单同时启用了现货和合约，请指定 inst_type".to_string(),
        )),
    }
}

pub(crate) async fn load_latest_candles_for_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<OkxCandle>> {
    let normalized_timeframe = timeframe.trim();
    super::super::market_ops::ensure_local_candles_for_read(
        state,
        inst_id,
        inst_type,
        normalized_timeframe,
        limit,
        false,
    )
    .await?;
    load_latest_candles_for_scope_from_db(
        &state.db,
        inst_id,
        inst_type,
        normalized_timeframe,
        limit,
    )
    .await
}

async fn load_latest_candles_for_scope_from_db(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<OkxCandle>> {
    load_latest_valid_okx_candles(db, inst_id, inst_type, timeframe, limit, "").await
}

pub(crate) async fn load_agent_candles(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<(String, String, Vec<OkxCandle>)> {
    let (normalized_id, normalized_type) = resolve_agent_scope(state, inst_id, inst_type).await?;
    let candles =
        load_latest_candles_for_scope(state, &normalized_id, &normalized_type, timeframe, limit)
            .await?;
    Ok((normalized_id, normalized_type, candles))
}

pub(crate) fn candle_ohlc_series(candles: &[OkxCandle]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for candle in candles {
        if candle.high > 0.0 && candle.low > 0.0 && candle.close > 0.0 {
            highs.push(candle.high);
            lows.push(candle.low);
            closes.push(candle.close);
        }
    }
    (highs, lows, closes)
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::load_latest_candles_for_scope_from_db;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
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
              volume_quote REAL,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles table");
        pool
    }

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str, volume_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, {volume_sql}, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn agent_scope_candles_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100", "1").await;
        insert_candle(&pool, 120_000, "101", "1").await;
        insert_candle(&pool, 180_000, "'bad-close'", "1").await;
        insert_candle(&pool, 240_000, "102", "'bad-volume'").await;

        let candles =
            load_latest_candles_for_scope_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
                .await
                .expect("load agent candles");
        let closes = candles
            .iter()
            .map(|candle| candle.close)
            .collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
