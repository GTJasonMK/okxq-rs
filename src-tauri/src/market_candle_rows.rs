use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite, SqlitePool};

use crate::{error::AppResult, ohlcv::Ohlcv, okx::OkxCandle};

const CANDLE_VALUE_COLUMNS: &str =
    "timestamp, open, high, low, close, volume, volume_ccy, volume_quote";
const CANDLE_ID_VALUE_COLUMNS: &str =
    "inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy, volume_quote";
const BASIC_CANDLE_VALUE_COLUMNS: &str = "timestamp, open, high, low, close, volume";
const VALID_BASIC_MARKET_CANDLE_FILTER_SQL: &str = r#"
            AND timestamp > 0
            AND typeof(open) IN ('integer', 'real') AND open > 0
            AND typeof(high) IN ('integer', 'real') AND high > 0
            AND typeof(low) IN ('integer', 'real') AND low > 0
            AND typeof(close) IN ('integer', 'real') AND close > 0
            AND typeof(volume) IN ('integer', 'real') AND volume >= 0
            AND typeof(volume_ccy) IN ('integer', 'real') AND volume_ccy >= 0
"#;
const VALID_QUOTE_VOLUME_FILTER_SQL: &str = r#"
            AND typeof(volume_quote) IN ('integer', 'real') AND volume_quote >= 0
"#;

pub(crate) struct ValidCandleStats {
    pub oldest_timestamp: Option<i64>,
    pub newest_timestamp: Option<i64>,
    pub candle_count: i64,
}

pub(crate) async fn load_latest_valid_candle_rows(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<SqliteRow>> {
    let sql = format!(
        r#"
        SELECT {CANDLE_ID_VALUE_COLUMNS}
        FROM (
          SELECT {CANDLE_ID_VALUE_COLUMNS}
          FROM candles
          WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
            {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
            {VALID_QUOTE_VOLUME_FILTER_SQL}
          ORDER BY timestamp DESC
          LIMIT ?
        )
        ORDER BY timestamp ASC
        "#,
    );
    sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(limit)
        .fetch_all(db)
        .await
        .map_err(Into::into)
}

pub(crate) async fn load_latest_valid_candle_json(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<Value>> {
    Ok(
        load_latest_valid_candle_rows(db, inst_id, inst_type, timeframe, limit)
            .await?
            .into_iter()
            .filter_map(|row| candle_json_from_row(&row))
            .collect(),
    )
}

pub(crate) async fn load_latest_valid_okx_candles(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
    confirm: &str,
) -> AppResult<Vec<OkxCandle>> {
    let rows = load_latest_valid_candle_rows(db, inst_id, inst_type, timeframe, limit).await?;
    Ok(okx_candles_from_rows(rows, confirm))
}

pub(crate) async fn load_latest_basic_candle_rows(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> Result<Vec<SqliteRow>, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT {BASIC_CANDLE_VALUE_COLUMNS}
        FROM (
          SELECT {BASIC_CANDLE_VALUE_COLUMNS}
          FROM candles
          WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
            {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
          ORDER BY timestamp DESC
          LIMIT ?
        )
        ORDER BY timestamp ASC
        "#,
    );
    sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(limit)
        .fetch_all(db)
        .await
}

pub(crate) async fn load_latest_basic_candle_closes(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> Result<Vec<f64>, sqlx::Error> {
    let rows = load_latest_basic_candle_rows(db, inst_id, inst_type, timeframe, limit).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| basic_candle_ohlcv_from_row(&row).map(|ohlcv| ohlcv.close))
        .collect())
}

pub(crate) async fn load_valid_candle_boundary_timestamp(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    descending: bool,
) -> AppResult<Option<i64>> {
    let order = if descending { "DESC" } else { "ASC" };
    let sql = format!(
        r#"
        SELECT timestamp
        FROM candles
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
          {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
        ORDER BY timestamp {order}
        LIMIT 1
        "#
    );
    let row = sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .fetch_optional(db)
        .await?;
    match row {
        Some(row) => Ok(Some(row.try_get::<i64, _>("timestamp")?)),
        None => Ok(None),
    }
}

pub(crate) async fn load_valid_candle_stats(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<ValidCandleStats> {
    let sql = format!(
        r#"
        SELECT MIN(timestamp) AS oldest_timestamp,
               MAX(timestamp) AS newest_timestamp,
               COUNT(*) AS candle_count
        FROM candles
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
          {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
        "#,
    );
    let row = sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .fetch_one(db)
        .await?;
    Ok(ValidCandleStats {
        oldest_timestamp: row.try_get::<Option<i64>, _>("oldest_timestamp")?,
        newest_timestamp: row.try_get::<Option<i64>, _>("newest_timestamp")?,
        candle_count: row.try_get::<i64, _>("candle_count")?,
    })
}

pub(crate) async fn count_valid_candle_rows_at_timestamps(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    timestamps: &[i64],
) -> AppResult<i64> {
    if timestamps.is_empty() {
        return Ok(0);
    }
    let mut total = 0i64;
    for chunk in timestamps.chunks(900) {
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT COUNT(*) AS existing_count
            FROM candles
            WHERE inst_id =
            "#,
        );
        query
            .push_bind(inst_id)
            .push(" AND inst_type = ")
            .push_bind(inst_type)
            .push(" AND timeframe = ")
            .push_bind(timeframe)
            .push(VALID_BASIC_MARKET_CANDLE_FILTER_SQL)
            .push(" AND timestamp IN (");
        for (index, timestamp) in chunk.iter().enumerate() {
            if index > 0 {
                query.push(", ");
            }
            query.push_bind(timestamp);
        }
        query.push(")");
        let row = query.build().fetch_one(db).await?;
        total += row.try_get::<i64, _>("existing_count")?;
    }
    Ok(total)
}

pub(crate) async fn load_valid_candle_rows_since(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    min_timestamp: Option<i64>,
) -> AppResult<Vec<SqliteRow>> {
    let min_timestamp_filter = if min_timestamp.is_some() {
        "AND timestamp >= ?"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT {CANDLE_VALUE_COLUMNS}
        FROM candles
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
          {min_timestamp_filter}
          {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
          {VALID_QUOTE_VOLUME_FILTER_SQL}
        ORDER BY timestamp ASC
        "#,
    );
    let mut query = sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe);
    if let Some(min_timestamp) = min_timestamp {
        query = query.bind(min_timestamp);
    }
    query.fetch_all(db).await.map_err(Into::into)
}

pub(crate) async fn load_valid_candle_rows_in_range(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    start_ts: i64,
    end_ts: i64,
    limit: Option<i64>,
) -> AppResult<Vec<SqliteRow>> {
    let limit_clause = if limit.is_some() { "LIMIT ?" } else { "" };
    let sql = format!(
        r#"
        SELECT {CANDLE_VALUE_COLUMNS}
        FROM candles
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
          AND timestamp >= ? AND timestamp <= ?
          {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
          {VALID_QUOTE_VOLUME_FILTER_SQL}
        ORDER BY timestamp ASC
        {limit_clause}
        "#,
    );
    let mut query = sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(start_ts)
        .bind(end_ts);
    if let Some(limit) = limit {
        query = query.bind(limit);
    }
    query.fetch_all(db).await.map_err(Into::into)
}

pub(crate) async fn load_recent_valid_candle_rows_until(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    end_ts: i64,
    limit: i64,
) -> AppResult<Vec<SqliteRow>> {
    let sql = format!(
        r#"
        SELECT {CANDLE_VALUE_COLUMNS}
        FROM (
          SELECT {CANDLE_VALUE_COLUMNS}
          FROM candles
          WHERE inst_id = ? AND inst_type = ? AND timeframe = ? AND timestamp <= ?
            {VALID_BASIC_MARKET_CANDLE_FILTER_SQL}
            {VALID_QUOTE_VOLUME_FILTER_SQL}
          ORDER BY timestamp DESC
          LIMIT ?
        )
        ORDER BY timestamp ASC
        "#,
    );
    sqlx::query(&sql)
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(end_ts)
        .bind(limit)
        .fetch_all(db)
        .await
        .map_err(Into::into)
}

pub(crate) fn okx_candle_from_row(row: &SqliteRow, confirm: &str) -> Option<OkxCandle> {
    let candle = OkxCandle {
        timestamp: positive_row_i64(row, "timestamp")?,
        open: positive_row_f64(row, "open")?,
        high: positive_row_f64(row, "high")?,
        low: positive_row_f64(row, "low")?,
        close: positive_row_f64(row, "close")?,
        volume: non_negative_row_f64(row, "volume")?,
        volume_ccy: non_negative_row_f64(row, "volume_ccy")?,
        volume_quote: non_negative_row_f64(row, "volume_quote")?,
        confirm: confirm.to_string(),
    };
    candle.is_valid_market_candle().then_some(candle)
}

pub(crate) fn okx_candles_from_rows(rows: Vec<SqliteRow>, confirm: &str) -> Vec<OkxCandle> {
    let mut candles = rows
        .into_iter()
        .filter_map(|row| okx_candle_from_row(&row, confirm))
        .collect::<Vec<_>>();
    candles.sort_by_key(|item| item.timestamp);
    candles.dedup_by_key(|item| item.timestamp);
    candles
}

pub(crate) fn candle_json_from_row(row: &SqliteRow) -> Option<Value> {
    let candle = okx_candle_from_row(row, "1")?;
    Some(json!({
        "inst_id": row.try_get::<String, _>("inst_id").ok()?,
        "inst_type": row.try_get::<String, _>("inst_type").ok()?,
        "timeframe": row.try_get::<String, _>("timeframe").ok()?,
        "timestamp": candle.timestamp,
        "open": candle.open,
        "high": candle.high,
        "low": candle.low,
        "close": candle.close,
        "volume": candle.volume,
        "volume_ccy": candle.volume_ccy,
        "volume_quote": candle.volume_quote,
    }))
}

pub(crate) fn positive_row_i64(row: &SqliteRow, column: &str) -> Option<i64> {
    row.try_get::<i64, _>(column)
        .ok()
        .filter(|value| *value > 0)
}

pub(crate) fn positive_row_f64(row: &SqliteRow, column: &str) -> Option<f64> {
    let value = row.try_get::<f64, _>(column).ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

pub(crate) fn non_negative_row_f64(row: &SqliteRow, column: &str) -> Option<f64> {
    let value = row.try_get::<f64, _>(column).ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

pub(crate) fn basic_candle_ohlcv_from_row(row: &SqliteRow) -> Option<Ohlcv> {
    Some(Ohlcv {
        open: positive_row_f64(row, "open")?,
        high: positive_row_f64(row, "high")?,
        low: positive_row_f64(row, "low")?,
        close: positive_row_f64(row, "close")?,
        volume: non_negative_row_f64(row, "volume")?,
    })
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn okx_candle_row_rejects_invalid_required_price() {
        let row = test_row(
            r#"
            SELECT
              1700000000000 AS timestamp,
              'bad-open' AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume,
              1200.0 AS volume_ccy,
              1200.0 AS volume_quote
            "#,
        )
        .await;

        assert!(okx_candle_from_row(&row, "1").is_none());
    }

    #[tokio::test]
    async fn basic_candle_ohlcv_row_rejects_invalid_required_price() {
        let row = test_row(
            r#"
            SELECT
              'bad-open' AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume
            "#,
        )
        .await;

        assert!(basic_candle_ohlcv_from_row(&row).is_none());
    }

    #[tokio::test]
    async fn basic_candle_ohlcv_row_rejects_invalid_volume() {
        let row = test_row(
            r#"
            SELECT
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              'bad-volume' AS volume
            "#,
        )
        .await;

        assert!(basic_candle_ohlcv_from_row(&row).is_none());
    }

    #[tokio::test]
    async fn candle_json_row_preserves_quote_volume_fields() {
        let row = test_row(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              '1m' AS timeframe,
              1700000000000 AS timestamp,
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              100.5 AS close,
              12.0 AS volume,
              1200.0 AS volume_ccy,
              1210.0 AS volume_quote
            "#,
        )
        .await;

        let value = candle_json_from_row(&row).expect("valid candle json");
        assert_eq!(value["inst_id"], "BTC-USDT-SWAP");
        assert_eq!(value["inst_type"], "SWAP");
        assert_eq!(value["timeframe"], "1m");
        assert_eq!(value["volume_quote"], 1210.0);
    }

    async fn test_row(sql: &str) -> SqliteRow {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(sql).fetch_one(&pool).await.expect("test row")
    }
}
