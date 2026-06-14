use sqlx::{query::Query, sqlite::SqliteArguments, Sqlite, SqlitePool};

use crate::{error::AppResult, strategy_engine::BacktestReport};

struct BacktestReportJsonFields {
    params_json: String,
    detail_json: String,
}

pub(crate) async fn insert_backtest_result(
    db: &SqlitePool,
    report: &BacktestReport,
) -> AppResult<i64> {
    let json_fields = backtest_report_json_fields(report)?;
    let result = bind_backtest_report_fields(
        sqlx::query(
            r#"
            INSERT INTO backtest_results (
              strategy_name, strategy_id, symbol, inst_type, timeframe, days,
              start_time, end_time, initial_capital, final_capital,
              total_return, annual_return, max_drawdown,
              sharpe_ratio, sortino_ratio, calmar_ratio,
              win_rate, profit_factor,
              total_trades, winning_trades, losing_trades,
              avg_profit, avg_loss, largest_profit, largest_loss,
              total_commission, params_json, detail_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        ),
        report,
        &json_fields,
    )
    .bind(&report.created_at)
    .execute(db)
    .await?;
    Ok(result.last_insert_rowid())
}

fn backtest_report_json_fields(report: &BacktestReport) -> AppResult<BacktestReportJsonFields> {
    Ok(BacktestReportJsonFields {
        params_json: serde_json::to_string(&report.params)?,
        detail_json: serde_json::to_string(&report.detail)?,
    })
}

fn bind_backtest_report_fields<'args>(
    query: Query<'args, Sqlite, SqliteArguments<'args>>,
    report: &'args BacktestReport,
    json_fields: &'args BacktestReportJsonFields,
) -> Query<'args, Sqlite, SqliteArguments<'args>> {
    query
        .bind(&report.strategy_name)
        .bind(&report.strategy_id)
        .bind(&report.symbol)
        .bind(&report.inst_type)
        .bind(&report.timeframe)
        .bind(report.days)
        .bind(&report.start_time)
        .bind(&report.end_time)
        .bind(report.initial_capital)
        .bind(report.final_capital)
        .bind(report.total_return)
        .bind(report.annual_return)
        .bind(report.max_drawdown)
        .bind(report.sharpe_ratio)
        .bind(report.sortino_ratio)
        .bind(report.calmar_ratio)
        .bind(report.win_rate)
        .bind(report.profit_factor)
        .bind(report.total_trades)
        .bind(report.winning_trades)
        .bind(report.losing_trades)
        .bind(report.avg_profit)
        .bind(report.avg_loss)
        .bind(report.largest_profit)
        .bind(report.largest_loss)
        .bind(report.total_commission)
        .bind(&json_fields.params_json)
        .bind(&json_fields.detail_json)
}
