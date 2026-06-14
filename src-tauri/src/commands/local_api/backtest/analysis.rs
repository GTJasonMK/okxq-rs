use serde_json::{json, Value};
use sqlx::Row;

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, request_f64, request_i64, LocalApiRequest},
    error::{AppError, AppResult},
};

pub(in crate::commands::local_api) async fn run_monte_carlo_analysis(
    state: &AppState,
    result_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let id = result_id
        .parse::<i64>()
        .map_err(|_| AppError::Validation("回测记录 ID 无效".to_string()))?;

    let row = sqlx::query("SELECT detail_json, initial_capital FROM backtest_results WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;

    let Some(row) = row else {
        return Ok(json!({"code": 404, "message": "回测记录不存在"}));
    };

    let detail_json = row.try_get::<String, _>("detail_json")?;
    let detail = parse_analysis_detail_json(&detail_json)?;
    let initial_capital = required_initial_capital(row.try_get::<f64, _>("initial_capital")?)?;

    let trade_pnls = detail_trade_cashflows(&detail)?;

    let num_simulations = request_i64(req, "num_simulations", 1000).clamp(100, 10_000) as usize;
    let block_size = request_i64(req, "block_size", 5).clamp(1, 500) as usize;
    let config = crate::monte_carlo::MonteCarloConfig {
        num_simulations,
        block_size,
        ..Default::default()
    };

    let result = crate::monte_carlo::run_monte_carlo(&trade_pnls, initial_capital, &config);
    Ok(code_ok(json!({
        "result_id": id,
        "num_trades": trade_pnls.len(),
        "initial_capital": initial_capital,
        "analysis": result.to_value()
    })))
}

pub(in crate::commands::local_api) async fn run_walk_forward_analysis(
    state: &AppState,
    result_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let id = result_id
        .parse::<i64>()
        .map_err(|_| AppError::Validation("回测记录 ID 无效".to_string()))?;

    let row = sqlx::query("SELECT detail_json FROM backtest_results WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;

    let Some(row) = row else {
        return Ok(json!({"code": 404, "message": "回测记录不存在"}));
    };

    let detail_json = row.try_get::<String, _>("detail_json")?;
    let detail = parse_analysis_detail_json(&detail_json)?;
    let config = crate::walk_forward::WalkForwardConfig {
        window_days: request_i64(req, "window_days", 30).clamp(1, 3650),
        step_days: request_i64(req, "step_days", 30).clamp(1, 3650),
        min_points: request_i64(req, "min_points", 3).clamp(2, 100_000) as usize,
        benchmark_sharpe: request_f64(req, "benchmark_sharpe", 0.0).clamp(-10.0, 10.0),
        trial_count: request_i64(req, "trial_count", 1).clamp(1, 1_000_000) as usize,
    };
    let analysis = crate::walk_forward::analyze_backtest_detail(&detail, &config);

    Ok(code_ok(json!({
        "result_id": id,
        "analysis": analysis
    })))
}

fn parse_analysis_detail_json(text: &str) -> AppResult<Value> {
    let detail = serde_json::from_str::<Value>(text)?;
    if detail.is_object() {
        Ok(detail)
    } else {
        Err(AppError::Runtime(
            "回测记录 detail_json 不是 JSON 对象".to_string(),
        ))
    }
}

fn required_initial_capital(value: f64) -> AppResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AppError::Runtime(
            "backtest result initial_capital 不是有限数字".to_string(),
        ))
    }
}

fn trade_cashflow(trade: &Value) -> AppResult<Option<f64>> {
    let cashflow = trade
        .get("cashflow")
        .and_then(Value::as_f64)
        .ok_or_else(|| AppError::Runtime("trade_cashflows 缺少 cashflow 数字".to_string()))?;
    if !cashflow.is_finite() {
        return Err(AppError::Runtime(
            "trade_cashflows.cashflow 不是有限数字".to_string(),
        ));
    }
    Ok((cashflow != 0.0).then_some(cashflow))
}

fn detail_trade_cashflows(detail: &Value) -> AppResult<Vec<f64>> {
    let rows = detail
        .get("trade_cashflows")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Runtime("回测记录 detail_json 缺少 trade_cashflows 数组".to_string())
        })?;
    let mut cashflows = Vec::new();
    for row in rows {
        if let Some(cashflow) = trade_cashflow(row)? {
            cashflows.push(cashflow);
        }
    }
    Ok(cashflows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monte_carlo_cashflow_includes_funding_events() {
        assert_eq!(
            trade_cashflow(&json!({"cashflow": -0.12})).expect("cashflow"),
            Some(-0.12)
        );
        assert_eq!(
            trade_cashflow(&json!({"cashflow": 2.8})).expect("cashflow"),
            Some(2.8)
        );
        assert_eq!(
            trade_cashflow(&json!({"cashflow": 0.0})).expect("cashflow"),
            None
        );
    }

    #[test]
    fn monte_carlo_cashflows_prefer_full_lightweight_rows() {
        let detail = json!({
            "trade_cashflows": [
                {"timestamp": 1, "cashflow": 1.0},
                {"timestamp": 2, "cashflow": 1.5}
            ],
            "trades": [
                {"timestamp": 3, "pnl": 999.0}
            ]
        });

        assert_eq!(
            detail_trade_cashflows(&detail).expect("cashflows"),
            vec![1.0, 1.5]
        );
    }

    #[test]
    fn monte_carlo_cashflows_reject_missing_lightweight_rows() {
        let detail = json!({
            "trades": [
                {"timestamp": 1, "pnl": 1.0, "funding": 0.5}
            ]
        });

        let error = detail_trade_cashflows(&detail).unwrap_err();

        assert!(error.to_string().contains("trade_cashflows"));
    }

    #[test]
    fn monte_carlo_cashflows_reject_missing_cashflow_field() {
        let error = trade_cashflow(&json!({"pnl": 1.0, "funding": 0.5})).unwrap_err();

        assert!(error.to_string().contains("cashflow"));
    }
}
