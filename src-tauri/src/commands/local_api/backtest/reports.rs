use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use crate::{
    backtest_result_persistence::insert_backtest_result, error::AppResult,
    strategy_engine::BacktestReport,
};

use super::integrity::{result_integrity_from_detail, result_integrity_from_summary_columns};

pub(super) async fn backtest_report_value(
    db: &sqlx::SqlitePool,
    report: &BacktestReport,
    persist_result: bool,
) -> AppResult<Value> {
    if persist_result {
        persist_backtest_report_to_db(db, report).await
    } else {
        backtest_report_value_with_integrity(report, None)
    }
}

async fn persist_backtest_report_to_db(
    db: &sqlx::SqlitePool,
    report: &BacktestReport,
) -> AppResult<Value> {
    let result_id = insert_backtest_result(db, report).await?;
    backtest_report_value_with_integrity(report, Some(result_id))
}

fn backtest_report_value_with_integrity(
    report: &BacktestReport,
    id: Option<i64>,
) -> AppResult<Value> {
    let mut value = report.to_value(id)?;
    let Some(integrity) = result_integrity_from_detail(&report.detail) else {
        return Ok(value);
    };
    if let Some(object) = value.as_object_mut() {
        object
            .entry("backtest_result_integrity".to_string())
            .or_insert(integrity);
    }
    Ok(value)
}

pub(in crate::commands::local_api::backtest) fn backtest_summary_row(
    row: SqliteRow,
) -> AppResult<Value> {
    let mut value = backtest_summary_value(&row)?;
    if let Some(integrity) = result_integrity_from_summary_columns(&row) {
        if let Some(object) = value.as_object_mut() {
            object.insert("backtest_result_integrity".to_string(), integrity);
        }
    }
    Ok(value)
}

pub(in crate::commands::local_api::backtest) fn backtest_detail_row(
    row: SqliteRow,
) -> AppResult<Value> {
    let mut value = backtest_summary_value(&row)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "detail_json".to_string(),
            Value::String(row.try_get::<String, _>("detail_json")?),
        );
    }
    Ok(value)
}

fn backtest_summary_value(row: &SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "strategy_name": row.try_get::<String, _>("strategy_name")?,
        "strategy_id": row.try_get::<String, _>("strategy_id")?,
        "symbol": row.try_get::<String, _>("symbol")?,
        "inst_type": row.try_get::<String, _>("inst_type")?,
        "timeframe": row.try_get::<String, _>("timeframe")?,
        "days": row.try_get::<i64, _>("days")?,
        "start_time": row.try_get::<Option<String>, _>("start_time")?,
        "end_time": row.try_get::<Option<String>, _>("end_time")?,
        "initial_capital": required_finite_f64(row, "initial_capital")?,
        "final_capital": required_finite_f64(row, "final_capital")?,
        "total_return": optional_finite_f64(row, "total_return")?,
        "annual_return": optional_finite_f64(row, "annual_return")?,
        "max_drawdown": optional_finite_f64(row, "max_drawdown")?,
        "sharpe_ratio": optional_finite_f64(row, "sharpe_ratio")?,
        "sortino_ratio": optional_finite_f64(row, "sortino_ratio")?,
        "calmar_ratio": optional_finite_f64(row, "calmar_ratio")?,
        "win_rate": optional_finite_f64(row, "win_rate")?,
        "profit_factor": optional_finite_f64(row, "profit_factor")?,
        "total_trades": row.try_get::<Option<i64>, _>("total_trades")?,
        "winning_trades": row.try_get::<Option<i64>, _>("winning_trades")?,
        "losing_trades": row.try_get::<Option<i64>, _>("losing_trades")?,
        "avg_profit": optional_finite_f64(row, "avg_profit")?,
        "avg_loss": optional_finite_f64(row, "avg_loss")?,
        "largest_profit": optional_finite_f64(row, "largest_profit")?,
        "largest_loss": optional_finite_f64(row, "largest_loss")?,
        "total_commission": optional_finite_f64(row, "total_commission")?,
        "params_json": row.try_get::<Option<String>, _>("params_json")?,
        "created_at": row.try_get::<Option<String>, _>("created_at")?,
    }))
}

fn required_finite_f64(row: &SqliteRow, column: &str) -> AppResult<f64> {
    let value = row.try_get::<f64, _>(column)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(crate::error::AppError::Runtime(format!(
            "backtest result {column} 不是有限数字"
        )))
    }
}

fn optional_finite_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let Some(value) = row.try_get::<Option<f64>, _>(column)? else {
        return Ok(None);
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(crate::error::AppError::Runtime(format!(
            "backtest result {column} 不是有限数字"
        )))
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn backtest_summary_row_rejects_invalid_required_capital() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              1 AS id,
              'Strategy' AS strategy_name,
              'strategy_a' AS strategy_id,
              'BTC-USDT-SWAP' AS symbol,
              'SWAP' AS inst_type,
              '1H' AS timeframe,
              30 AS days,
              NULL AS start_time,
              NULL AS end_time,
              'bad-capital' AS initial_capital,
              1010.0 AS final_capital,
              NULL AS total_return,
              NULL AS annual_return,
              NULL AS max_drawdown,
              NULL AS sharpe_ratio,
              NULL AS sortino_ratio,
              NULL AS calmar_ratio,
              NULL AS win_rate,
              NULL AS profit_factor,
              NULL AS total_trades,
              NULL AS winning_trades,
              NULL AS losing_trades,
              NULL AS avg_profit,
              NULL AS avg_loss,
              NULL AS largest_profit,
              NULL AS largest_loss,
              NULL AS total_commission,
              NULL AS params_json,
              NULL AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("summary row");

        assert!(backtest_summary_row(row).is_err());
    }
}
