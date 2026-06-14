use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use crate::error::{AppError, AppResult};

use super::super::values::finite_text_f64;

pub(crate) fn fill_row_to_json(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?.to_string(),
        "trade_id": optional_string(&row, "trade_id")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "ccy": row.try_get::<String, _>("ccy")?,
        "side": row.try_get::<String, _>("side")?,
        "fill_px": text_f64_column(&row, "fill_px")?,
        "fill_sz": text_f64_column(&row, "fill_sz")?,
        "fee": text_f64_column(&row, "fee")?,
        "fee_ccy": optional_string(&row, "fee_ccy")?,
        "ts": row.try_get::<i64, _>("ts")?,
        "mode": row.try_get::<String, _>("mode")?,
        "source": row.try_get::<String, _>("source")?,
        "order_id": row.try_get::<String, _>("order_id")?,
        "client_order_id": row.try_get::<String, _>("client_order_id")?,
        "strategy_id": row.try_get::<String, _>("strategy_id")?,
        "run_id": row.try_get::<String, _>("run_id")?,
        "arrival_ts": row.try_get::<Option<i64>, _>("arrival_ts")?,
        "arrival_mid_px": optional_finite_f64(&row, "arrival_mid_px")?,
        "arrival_bid_px": optional_finite_f64(&row, "arrival_bid_px")?,
        "arrival_ask_px": optional_finite_f64(&row, "arrival_ask_px")?,
        "created_at": row.try_get::<Option<String>, _>("created_at")?
    }))
}

fn optional_string(row: &SqliteRow, column: &str) -> AppResult<String> {
    Ok(row
        .try_get::<Option<String>, _>(column)?
        .unwrap_or_default())
}

fn text_f64_column(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let text = row.try_get::<String, _>(column)?;
    Ok(finite_text_f64(&text))
}

fn optional_finite_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let Some(value) = row.try_get::<Option<f64>, _>(column)? else {
        return Ok(None);
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(AppError::Runtime(format!(
            "local fill {column} 不是有限数字"
        )))
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn fill_row_to_json_does_not_fabricate_zero_from_invalid_text_numbers() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let row = sqlx::query(
            r#"
            SELECT
              1 AS id,
              'trade-bad' AS trade_id,
              'BTC-USDT-SWAP' AS inst_id,
              'BTC' AS ccy,
              'buy' AS side,
              'bad-price' AS fill_px,
              'bad-size' AS fill_sz,
              'bad-fee' AS fee,
              'USDT' AS fee_ccy,
              1780000000000 AS ts,
              'simulated' AS mode,
              'api' AS source,
              '' AS order_id,
              '' AS client_order_id,
              '' AS strategy_id,
              '' AS run_id,
              NULL AS arrival_ts,
              NULL AS arrival_mid_px,
              NULL AS arrival_bid_px,
              NULL AS arrival_ask_px,
              NULL AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("row should be returned");

        let fill = fill_row_to_json(row).expect("fill row json");

        assert_eq!(fill["trade_id"], "trade-bad");
        assert!(fill["fill_px"].is_null());
        assert!(fill["fill_sz"].is_null());
        assert!(fill["fee"].is_null());
    }

    #[tokio::test]
    async fn fill_row_to_json_rejects_missing_required_columns() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let row = sqlx::query(
            r#"
            SELECT
              1 AS id,
              'trade-bad' AS trade_id,
              'BTC-USDT-SWAP' AS inst_id,
              'BTC' AS ccy,
              'buy' AS side,
              '100' AS fill_px,
              '1' AS fill_sz,
              '0' AS fee,
              'USDT' AS fee_ccy,
              1780000000000 AS ts,
              'simulated' AS mode,
              '' AS order_id,
              '' AS client_order_id,
              '' AS strategy_id,
              '' AS run_id,
              NULL AS arrival_ts,
              NULL AS arrival_mid_px,
              NULL AS arrival_bid_px,
              NULL AS arrival_ask_px,
              NULL AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("row should be returned");

        assert!(fill_row_to_json(row).is_err());
    }
}
