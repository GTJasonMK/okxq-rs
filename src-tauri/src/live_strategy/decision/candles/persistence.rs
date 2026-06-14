use std::collections::BTreeMap;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::{error::AppResult, market_candle_rows::load_latest_valid_okx_candles, okx::OkxCandle};

use super::CANDLE_UPSERT_CHUNK_SIZE;
use crate::live_strategy::runtime_helpers::canonical_timeframe;

pub(super) async fn persist_and_merge_candles(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    candles: Vec<OkxCandle>,
    by_ts: &mut BTreeMap<i64, OkxCandle>,
) -> AppResult<()> {
    if candles.is_empty() {
        return Ok(());
    }
    save_strategy_candles(db, symbol, inst_type, timeframe, &candles).await?;
    for candle in candles {
        by_ts.insert(candle.timestamp, candle);
    }
    Ok(())
}

pub(super) async fn save_strategy_candles(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    candles: &[OkxCandle],
) -> AppResult<i64> {
    let valid = candles
        .iter()
        .filter(|candle| candle.is_valid_market_candle())
        .collect::<Vec<_>>();
    let mut saved = 0_i64;
    for chunk in valid.chunks(CANDLE_UPSERT_CHUNK_SIZE) {
        let mut tx = db.begin().await?;
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            )
            "#,
        );
        query.push_values(chunk.iter(), |mut row, candle| {
            row.push_bind(symbol)
                .push_bind(inst_type)
                .push_bind(timeframe)
                .push_bind(candle.timestamp)
                .push_bind(candle.open)
                .push_bind(candle.high)
                .push_bind(candle.low)
                .push_bind(candle.close)
                .push_bind(candle.volume)
                .push_bind(candle.volume_ccy)
                .push_bind(candle.volume_quote);
        });
        query.push(
            r#"
            ON CONFLICT(inst_id, inst_type, timeframe, timestamp) DO UPDATE SET
              open = excluded.open,
              high = excluded.high,
              low = excluded.low,
              close = excluded.close,
              volume = excluded.volume,
              volume_ccy = excluded.volume_ccy,
              volume_quote = excluded.volume_quote
            WHERE candles.open IS NOT excluded.open
               OR candles.high IS NOT excluded.high
               OR candles.low IS NOT excluded.low
               OR candles.close IS NOT excluded.close
               OR candles.volume IS NOT excluded.volume
               OR candles.volume_ccy IS NOT excluded.volume_ccy
               OR candles.volume_quote IS NOT excluded.volume_quote
            "#,
        );
        let result = query.build().execute(&mut *tx).await?;
        saved += result.rows_affected() as i64;
        tx.commit().await?;
    }
    Ok(saved)
}

pub(super) async fn load_local_strategy_candles(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    limit: usize,
) -> AppResult<Vec<OkxCandle>> {
    let timeframe = canonical_timeframe(timeframe)
        .map(|value| value.to_string())
        .unwrap_or_else(|| timeframe.trim().to_string());
    load_latest_valid_okx_candles(db, symbol, inst_type, &timeframe, limit as i64, "1").await
}
