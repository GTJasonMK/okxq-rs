use serde_json::{json, Map, Value};

use crate::error::{AppError, AppResult};

#[derive(Clone, Debug)]
pub struct StrategyConfig {
    pub strategy_id: String,
    pub strategy_name: String,
    pub symbol: String,
    pub inst_type: String,
    pub timeframe: String,
    pub initial_capital: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub params: Value,
}

#[derive(Clone, Debug)]
pub struct StrategyActionRecord {
    pub action: String,
    pub side: String,
    pub price: f64,
    pub reason: String,
    pub strength: f64,
    pub timestamp: i64,
    pub position_size: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct BacktestReport {
    pub strategy_name: String,
    pub strategy_id: String,
    pub symbol: String,
    pub inst_type: String,
    pub timeframe: String,
    pub days: i64,
    pub start_time: String,
    pub end_time: String,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_return: f64,
    pub annual_return: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub omega_ratio: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub avg_profit: f64,
    pub avg_loss: f64,
    pub largest_profit: f64,
    pub largest_loss: f64,
    pub total_commission: f64,
    pub params: Value,
    pub detail: Value,
    pub sample_step: usize,
    pub created_at: String,
}

impl BacktestReport {
    pub fn to_value(&self, id: Option<i64>) -> AppResult<Value> {
        let mut detail = self.detail.as_object().cloned().ok_or_else(|| {
            AppError::Runtime("backtest report detail 不是 JSON 对象".to_string())
        })?;
        validate_detail_array(&detail, "trade_cashflows")?;
        validate_detail_u64(&detail, "trade_cashflows_total")?;
        validate_detail_bool(&detail, "equity_curve_sampled")?;
        validate_detail_u64(&detail, "equity_points_total")?;
        let candles = remove_detail_array(&mut detail, "candles")?;
        let equity_curve = remove_detail_array(&mut detail, "equity_curve")?;
        let trades = remove_detail_array(&mut detail, "trades")?;
        let trade_events_total = remove_detail_u64(&mut detail, "trade_events_total")?;
        let trades_truncated = remove_detail_bool(&mut detail, "trades_truncated")?;
        let indicators = remove_detail_object(&mut detail, "indicators")?;
        let strategy_actions = remove_detail_array(&mut detail, "strategy_actions")?;
        let strategy_diagnostics = remove_detail_object(&mut detail, "strategy_diagnostics")?;

        let mut value = json!({
            "id": id,
            "strategy_name": self.strategy_name,
            "strategy_id": self.strategy_id,
            "symbol": self.symbol,
            "inst_type": self.inst_type,
            "timeframe": self.timeframe,
            "days": self.days,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "initial_capital": self.initial_capital,
            "final_capital": self.final_capital,
            "total_return": self.total_return,
            "annual_return": self.annual_return,
            "max_drawdown": self.max_drawdown,
            "sharpe_ratio": self.sharpe_ratio,
            "sortino_ratio": self.sortino_ratio,
            "calmar_ratio": self.calmar_ratio,
            "omega_ratio": self.omega_ratio,
            "win_rate": self.win_rate,
            "profit_factor": self.profit_factor,
            "total_trades": self.total_trades,
            "winning_trades": self.winning_trades,
            "losing_trades": self.losing_trades,
            "avg_profit": self.avg_profit,
            "avg_loss": self.avg_loss,
            "largest_profit": self.largest_profit,
            "largest_loss": self.largest_loss,
            "total_commission": self.total_commission,
            "params": self.params,
            "sample_step": self.sample_step,
            "created_at": self.created_at,
            "candles": candles,
            "equity_curve": equity_curve,
            "trades": trades,
            "trade_events_total": trade_events_total,
            "trades_truncated": trades_truncated,
            "indicators": indicators,
            "strategy_actions": strategy_actions,
            "strategy_diagnostics": strategy_diagnostics
        });
        merge_extra_detail_fields(&mut value, &detail);
        Ok(value)
    }
}

fn remove_detail_array(detail: &mut Map<String, Value>, key: &str) -> AppResult<Value> {
    let value = remove_detail_field(detail, key)?;
    if value.is_array() {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "backtest report detail.{key} 不是 JSON 数组"
        )))
    }
}

fn remove_detail_object(detail: &mut Map<String, Value>, key: &str) -> AppResult<Value> {
    let value = remove_detail_field(detail, key)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "backtest report detail.{key} 不是 JSON 对象"
        )))
    }
}

fn remove_detail_bool(detail: &mut Map<String, Value>, key: &str) -> AppResult<Value> {
    let value = remove_detail_field(detail, key)?;
    value
        .as_bool()
        .map(Value::Bool)
        .ok_or_else(|| AppError::Runtime(format!("backtest report detail.{key} 不是布尔值")))
}

fn remove_detail_u64(detail: &mut Map<String, Value>, key: &str) -> AppResult<Value> {
    let value = remove_detail_field(detail, key)?;
    value
        .as_u64()
        .map(|count| json!(count))
        .ok_or_else(|| AppError::Runtime(format!("backtest report detail.{key} 不是非负整数")))
}

fn remove_detail_field(detail: &mut Map<String, Value>, key: &str) -> AppResult<Value> {
    detail
        .remove(key)
        .ok_or_else(|| AppError::Runtime(format!("backtest report detail 缺少 {key}")))
}

fn validate_detail_array(detail: &Map<String, Value>, key: &str) -> AppResult<()> {
    validate_detail_field(detail, key, Value::is_array, "JSON 数组")
}

fn validate_detail_bool(detail: &Map<String, Value>, key: &str) -> AppResult<()> {
    validate_detail_field(detail, key, Value::is_boolean, "布尔值")
}

fn validate_detail_u64(detail: &Map<String, Value>, key: &str) -> AppResult<()> {
    let value = detail
        .get(key)
        .ok_or_else(|| AppError::Runtime(format!("backtest report detail 缺少 {key}")))?;
    if value.as_u64().is_some() {
        Ok(())
    } else {
        Err(AppError::Runtime(format!(
            "backtest report detail.{key} 不是非负整数"
        )))
    }
}

fn validate_detail_field(
    detail: &Map<String, Value>,
    key: &str,
    valid: impl Fn(&Value) -> bool,
    expected: &str,
) -> AppResult<()> {
    let value = detail
        .get(key)
        .ok_or_else(|| AppError::Runtime(format!("backtest report detail 缺少 {key}")))?;
    if valid(value) {
        Ok(())
    } else {
        Err(AppError::Runtime(format!(
            "backtest report detail.{key} 不是 {expected}"
        )))
    }
}

fn merge_extra_detail_fields(target: &mut Value, detail: &Map<String, Value>) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    for (key, value) in detail {
        if value.is_null() || target.contains_key(key) {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_with_detail(detail: Value) -> BacktestReport {
        BacktestReport {
            strategy_name: "Strategy".to_string(),
            strategy_id: "strategy_a".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "1H".to_string(),
            days: 1,
            start_time: "2026-01-01T00:00:00Z".to_string(),
            end_time: "2026-01-02T00:00:00Z".to_string(),
            initial_capital: 10_000.0,
            final_capital: 10_000.0,
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
            params: json!({}),
            detail,
            sample_step: 1,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        }
    }

    fn complete_detail() -> Value {
        json!({
            "candles": [],
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
        })
    }

    #[test]
    fn backtest_report_to_value_rejects_missing_generated_detail_field() {
        let mut detail = complete_detail();
        detail
            .as_object_mut()
            .expect("detail object")
            .remove("trade_cashflows");

        let error = report_with_detail(detail).to_value(None).unwrap_err();

        assert!(error.to_string().contains("trade_cashflows"));
    }

    #[test]
    fn backtest_report_to_value_rejects_invalid_generated_detail_field_type() {
        let mut detail = complete_detail();
        detail["trade_events_total"] = json!(-1);

        let error = report_with_detail(detail).to_value(None).unwrap_err();

        assert!(error.to_string().contains("trade_events_total"));
    }
}

#[derive(Clone)]
pub(super) struct TradeRecord {
    pub(super) symbol: Option<String>,
    pub(super) timestamp: i64,
    pub(super) side: String,
    pub(super) pos_side: Option<String>,
    pub(super) action: Option<String>,
    pub(super) price: f64,
    pub(super) quantity: f64,
    pub(super) exchange_quantity: f64,
    pub(super) value: f64,
    pub(super) commission: f64,
    pub(super) pnl: Option<f64>,
    pub(super) funding: f64,
    pub(super) equity: Option<f64>,
    pub(super) reason: String,
}
