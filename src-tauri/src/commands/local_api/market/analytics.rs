use serde_json::{json, Value};

use crate::{
    app_state::AppState, correlation, error::AppResult,
    market_candle_rows::load_latest_valid_okx_candles, okx::OkxCandle,
};

use super::super::market_ops;
use super::super::*;

pub(in crate::commands::local_api) async fn market_indicators(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let timeframe = body_string(req, "timeframe", "1H");
    let limit = body_i64(req, "limit", 100).clamp(20, 1000);
    let (inst_id, inst_type) = resolve_watched_market_inst(state, &inst_id, &inst_type).await?;
    let sync_result = market_ops::ensure_local_candles_for_read(
        state, &inst_id, &inst_type, &timeframe, limit, false,
    )
    .await?;
    let candles =
        load_indicator_candles(&state.db, &inst_id, &inst_type, &timeframe, limit).await?;
    let calculator = crate::indicators::IndicatorCalculator::new(&candles);
    let mut indicators = calculator.calculate_all();
    if let Some(obj) = indicators.as_object_mut() {
        obj.insert("inst_id".to_string(), Value::String(inst_id));
        if let Some(sync_result) = sync_result {
            obj.insert("sync_result".to_string(), sync_result);
        }
    }
    Ok(code_ok(indicators))
}

async fn load_indicator_candles(
    db: &sqlx::SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<OkxCandle>> {
    load_latest_valid_okx_candles(db, inst_id, inst_type, timeframe, limit, "").await
}

pub(in crate::commands::local_api) async fn market_correlation(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_ids = request_string_array(req, "inst_ids");
    let default_inst_type = inst_ids
        .first()
        .map(|inst_id| infer_inst_type(inst_id))
        .unwrap_or_else(|| "SPOT".to_string());
    let inst_type = body_string(req, "inst_type", &default_inst_type);
    let timeframe = body_string(req, "timeframe", "1H");
    let lookback = body_i64(req, "lookback", 100).clamp(20, 200);
    if inst_ids.len() < 2 {
        return Ok(code_ok(json!({
            "symbols": [],
            "matrix": [],
            "top_positive": [],
            "lowest": [],
            "data_points": 0,
            "message": "请至少提供两个币种"
        })));
    }
    let normalized_ids = inst_ids
        .iter()
        .map(|inst_id| normalize_market_inst_id(inst_id, &inst_type))
        .collect::<Vec<_>>();
    for inst_id in &normalized_ids {
        market_ops::ensure_local_candles_for_read(
            state, inst_id, &inst_type, &timeframe, lookback, false,
        )
        .await?;
    }
    correlation::compute_correlation_matrix(
        state,
        &normalized_ids,
        &inst_type,
        &timeframe,
        lookback,
    )
    .await
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::load_indicator_candles;
    use crate::market_candle_rows::okx_candle_from_row;

    #[tokio::test]
    async fn indicator_candle_row_does_not_fabricate_zero_from_invalid_required_price_text() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              1700000000000 AS timestamp,
              'bad-open' AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume,
              1200.0 AS volume_ccy
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(okx_candle_from_row(&row, "").is_none());
    }

    #[tokio::test]
    async fn indicator_candle_row_rejects_missing_volume_ccy_instead_of_fabricating_zero() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              1700000000000 AS timestamp,
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(okx_candle_from_row(&row, "").is_none());
    }

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

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, 1, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn indicator_candles_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let candles = load_indicator_candles(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load indicator candles");
        let closes = candles
            .iter()
            .map(|candle| candle.close)
            .collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
