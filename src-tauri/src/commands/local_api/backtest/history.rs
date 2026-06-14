use serde_json::{json, Map, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, param_i64, param_string, LocalApiRequest},
    error::{AppError, AppResult},
};

use super::{
    integrity::result_integrity_from_detail,
    reports::{backtest_detail_row, backtest_summary_row},
};

pub(in crate::commands::local_api) async fn backtest_history(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 50).clamp(1, 200);
    let strategy_id = param_string(req, "strategy_id", "");
    let symbol = param_string(req, "symbol", "");
    let mut sql = String::from(
        r#"
        SELECT id, strategy_name, strategy_id, symbol, inst_type, timeframe, days,
               start_time, end_time, initial_capital, final_capital,
               total_return, annual_return, max_drawdown,
               sharpe_ratio, sortino_ratio, calmar_ratio,
               win_rate, profit_factor,
               total_trades, winning_trades, losing_trades,
               avg_profit, avg_loss, largest_profit, largest_loss,
               total_commission, params_json, created_at,
               json_type(detail_json, '$.runtime_action_summary') AS runtime_action_summary_type,
               json_array_length(detail_json, '$.strategy_actions') AS strategy_action_count,
               json_array_length(detail_json, '$.runtime_action_summary.warnings') AS runtime_summary_warning_count,
               json_extract(detail_json, '$.runtime_action_summary.planned_exit_contract') AS planned_exit_contract,
               json_extract(detail_json, '$.engine_version') AS engine_version,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.status') AS history_action_contract_status,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.open_action_count') AS history_open_action_count,
               json_extract(detail_json, '$.strategy_diagnostics.history_action_contract.open_actions_with_planned_exit') AS history_open_actions_with_planned_exit
        FROM backtest_results
        WHERE 1 = 1
        "#,
    );
    if !strategy_id.is_empty() {
        sql.push_str(" AND strategy_id = ?");
    }
    if !symbol.is_empty() {
        sql.push_str(" AND symbol = ?");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ?");
    let mut query = sqlx::query(&sql);
    if !strategy_id.is_empty() {
        query = query.bind(strategy_id);
    }
    if !symbol.is_empty() {
        query = query.bind(symbol);
    }
    query = query.bind(limit);
    let data = query
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(backtest_summary_row)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(code_ok(Value::Array(data)))
}

pub(in crate::commands::local_api) async fn backtest_detail(
    state: &AppState,
    result_id: &str,
) -> AppResult<Value> {
    let id = result_id
        .parse::<i64>()
        .map_err(|_| AppError::Validation("回测记录 ID 无效".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT id, strategy_name, strategy_id, symbol, inst_type, timeframe, days,
               start_time, end_time, initial_capital, final_capital,
               total_return, annual_return, max_drawdown,
               sharpe_ratio, sortino_ratio, calmar_ratio,
               win_rate, profit_factor,
               total_trades, winning_trades, losing_trades,
               avg_profit, avg_loss, largest_profit, largest_loss,
               total_commission, params_json, detail_json, created_at
        FROM backtest_results
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Ok(json!({"code": 404, "message": "回测记录不存在", "data": null}));
    };
    let mut result = backtest_detail_row(row)?;
    if let Some(obj) = result.as_object_mut() {
        let detail_json = obj
            .remove("detail_json")
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .ok_or_else(|| AppError::Runtime("回测记录缺少 detail_json".to_string()))?;
        let detail = parse_backtest_detail_json(&detail_json)?;
        append_backtest_detail_fields(obj, &detail)?;
    }
    Ok(code_ok(result))
}

fn parse_backtest_detail_json(text: &str) -> AppResult<Value> {
    let detail = serde_json::from_str::<Value>(text)?;
    if detail.is_object() {
        Ok(detail)
    } else {
        Err(AppError::Runtime(
            "回测记录 detail_json 不是 JSON 对象".to_string(),
        ))
    }
}

fn append_backtest_detail_fields(target: &mut Map<String, Value>, detail: &Value) -> AppResult<()> {
    let detail_object = detail
        .as_object()
        .ok_or_else(|| AppError::Runtime("回测记录 detail_json 不是 JSON 对象".to_string()))?;
    target.insert(
        "equity_curve".to_string(),
        required_detail_array(detail_object, "equity_curve")?,
    );
    target.insert(
        "trades".to_string(),
        required_detail_array(detail_object, "trades")?,
    );
    target.insert(
        "trade_events_total".to_string(),
        required_detail_u64(detail_object, "trade_events_total")?,
    );
    target.insert(
        "trades_truncated".to_string(),
        required_detail_bool(detail_object, "trades_truncated")?,
    );
    target.insert(
        "candles".to_string(),
        required_detail_array(detail_object, "candles")?,
    );
    target.insert(
        "indicators".to_string(),
        required_detail_object(detail_object, "indicators")?,
    );
    merge_detail_extras(target, detail_object);
    if let Some(integrity) = result_integrity_from_detail(detail) {
        target.insert("backtest_result_integrity".to_string(), integrity);
    }
    Ok(())
}

fn required_detail_field<'a>(detail: &'a Map<String, Value>, field: &str) -> AppResult<&'a Value> {
    detail
        .get(field)
        .ok_or_else(|| AppError::Runtime(format!("回测记录 detail_json 缺少 {field}")))
}

fn required_detail_array(detail: &Map<String, Value>, field: &str) -> AppResult<Value> {
    let value = required_detail_field(detail, field)?;
    if value.is_array() {
        Ok(value.clone())
    } else {
        Err(AppError::Runtime(format!(
            "回测记录 detail_json.{field} 不是 JSON 数组"
        )))
    }
}

fn required_detail_object(detail: &Map<String, Value>, field: &str) -> AppResult<Value> {
    let value = required_detail_field(detail, field)?;
    if value.is_object() {
        Ok(value.clone())
    } else {
        Err(AppError::Runtime(format!(
            "回测记录 detail_json.{field} 不是 JSON 对象"
        )))
    }
}

fn required_detail_bool(detail: &Map<String, Value>, field: &str) -> AppResult<Value> {
    let value = required_detail_field(detail, field)?;
    value
        .as_bool()
        .map(Value::Bool)
        .ok_or_else(|| AppError::Runtime(format!("回测记录 detail_json.{field} 不是布尔值")))
}

fn required_detail_u64(detail: &Map<String, Value>, field: &str) -> AppResult<Value> {
    let value = required_detail_field(detail, field)?;
    value
        .as_u64()
        .map(|count| json!(count))
        .ok_or_else(|| AppError::Runtime(format!("回测记录 detail_json.{field} 不是非负整数")))
}

fn merge_detail_extras(target: &mut Map<String, Value>, detail: &Map<String, Value>) {
    for (key, value) in detail {
        if value.is_null() || target.contains_key(key) {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
}

pub(in crate::commands::local_api) async fn delete_backtest_result(
    state: &AppState,
    result_id: &str,
) -> AppResult<Value> {
    let id = result_id
        .parse::<i64>()
        .map_err(|_| AppError::Validation("回测记录 ID 无效".to_string()))?;
    let deleted = sqlx::query("DELETE FROM backtest_results WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Ok(json!({"code": 404, "message": "回测记录不存在"}));
    }
    Ok(json!({"code": 0, "message": "已删除"}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_detail() -> Value {
        json!({
            "candles": [],
            "equity_curve": [],
            "trades": [],
            "trade_events_total": 0,
            "trades_truncated": false,
            "indicators": {},
            "strategy_actions": [],
            "strategy_diagnostics": {}
        })
    }

    #[test]
    fn append_backtest_detail_fields_rejects_missing_required_field() {
        let mut detail = complete_detail();
        detail
            .as_object_mut()
            .expect("detail object")
            .remove("trade_events_total");
        let mut target = Map::new();

        let error = append_backtest_detail_fields(&mut target, &detail).unwrap_err();

        assert!(error.to_string().contains("trade_events_total"));
    }

    #[test]
    fn append_backtest_detail_fields_rejects_invalid_required_field_type() {
        let mut detail = complete_detail();
        detail["trades_truncated"] = json!("false");
        let mut target = Map::new();

        let error = append_backtest_detail_fields(&mut target, &detail).unwrap_err();

        assert!(error.to_string().contains("trades_truncated"));
    }
}
