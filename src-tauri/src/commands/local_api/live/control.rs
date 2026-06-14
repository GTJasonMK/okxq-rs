use serde_json::{Map, Value};

use crate::{live_strategy::LiveStrategyConfig, strategy_executor};

use super::super::*;

pub(in crate::commands::local_api) async fn live_strategy_status(
    state: &AppState,
) -> AppResult<Value> {
    Ok(code_ok(serde_json::to_value(
        state.live_strategy.status().await,
    )?))
}

pub(in crate::commands::local_api) async fn live_execution_logs(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 120).clamp(1, 300) as usize;
    let requested_run_id = param_string(req, "run_id", "");
    let status = state.live_strategy.status().await;
    let mode = if has_explicit_mode(req) {
        super::request_live_strategy_mode(req, "simulated")?
    } else if !status.mode.trim().is_empty() {
        status.mode.clone()
    } else {
        request_trading_mode(state, req).await?
    };
    let run_id = if requested_run_id.trim().is_empty() {
        status.run_id
    } else {
        requested_run_id
    };
    let logs =
        crate::live_strategy::query_live_execution_logs(&state.db, limit as i64, &mode, &run_id)
            .await?;
    Ok(code_ok(serde_json::to_value(logs)?))
}

pub(in crate::commands::local_api) async fn start_live_strategy(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let strategy_id = body_string(req, "strategy_id", "");
    let meta = strategy_executor::ensure_registered(&state.paths.root, &strategy_id)?;
    let strategy_name = meta.strategy_name.clone();
    let runtime = &meta.runtime_config;
    let param_overrides = live_param_overrides(req)?;
    let raw_symbol = request_runtime_string(
        req,
        &param_overrides,
        "symbol",
        &backtest::runtime_string(runtime, "symbol", "BTC-USDT"),
    );
    let inferred_inst_type = infer_inst_type(&raw_symbol);
    let inst_type = request_runtime_string(
        req,
        &param_overrides,
        "inst_type",
        &backtest::runtime_string(runtime, "inst_type", &inferred_inst_type),
    )
    .to_uppercase();
    let symbol = backtest::normalize_strategy_inst_id(&raw_symbol, &inst_type)?;
    let mode = super::request_live_strategy_mode(
        req,
        &backtest::runtime_string(runtime, "mode", "simulated"),
    )?;

    let (risk_enabled, max_single_loss, max_position_pct, max_order_value) =
        load_risk_control_for_live(state, &mode).await;
    let params = strategy_executor::merge_default_params(&strategy_id, param_overrides.clone());
    let params_value = Value::Object(params.clone());

    let config = LiveStrategyConfig {
        strategy_id,
        strategy_name: strategy_name.clone(),
        symbol: symbol.clone(),
        timeframe: request_runtime_string(
            req,
            &param_overrides,
            "timeframe",
            &backtest::runtime_string(runtime, "timeframe", "1H"),
        ),
        inst_type,
        mode,
        initial_capital: request_runtime_f64(
            req,
            &param_overrides,
            "initial_capital",
            backtest::runtime_f64(runtime, "initial_capital", 10_000.0),
        ),
        position_size: request_runtime_f64(
            req,
            &param_overrides,
            "position_size",
            backtest::runtime_f64(runtime, "position_size", 0.2),
        ),
        stop_loss: request_runtime_f64(
            req,
            &param_overrides,
            "stop_loss",
            backtest::runtime_f64(runtime, "stop_loss", 0.05),
        ),
        take_profit: request_runtime_f64(
            req,
            &param_overrides,
            "take_profit",
            backtest::runtime_f64(runtime, "take_profit", 0.10),
        ),
        risk_timeframe: request_runtime_string(
            req,
            &param_overrides,
            "risk_timeframe",
            &backtest::runtime_string(runtime, "risk_timeframe", "1m"),
        ),
        check_interval: request_runtime_f64(
            req,
            &param_overrides,
            "check_interval",
            backtest::runtime_f64(runtime, "check_interval", 60.0),
        )
        .round()
        .max(1.0) as u64,
        params: params_value.clone(),
        project_root: state.paths.root.clone(),
        risk_control_enabled: request_runtime_bool(
            &param_overrides,
            "risk_control_enabled",
            risk_enabled,
        ),
        max_single_loss_ratio: request_runtime_f64_from_params(
            &param_overrides,
            "max_single_loss_ratio",
            max_single_loss,
        ),
        max_position_pct: request_runtime_f64_from_params(
            &param_overrides,
            "max_symbol_exposure_pct",
            max_position_pct,
        ),
        max_order_value: request_runtime_f64_from_params(
            &param_overrides,
            "max_order_value",
            max_order_value,
        ),
    };
    let client = crate::live_strategy::build_live_strategy_client(state).await?;
    let private_credentials =
        okx_private_credentials(state, &config.mode)
            .await
            .map_err(|error| {
                let mode_label = if config.mode == "live" {
                    "live OKX 实盘"
                } else {
                    "simulated OKX 模拟盘"
                };
                AppError::Validation(format!(
                    "实时策略需要 {mode_label} 私有 WebSocket 订阅权限: {error}"
                ))
            })?;
    let private_client =
        crate::live_strategy::build_live_strategy_private_client(state, &config.mode)
            .await
            .map_err(|error| {
                let mode_label = if config.mode == "live" {
                    "live OKX 实盘"
                } else {
                    "simulated OKX 模拟盘"
                };
                AppError::Validation(format!("实时策略需要 {mode_label} 私有交易权限: {error}"))
            })?;
    let status = state
        .live_strategy
        .start_with_private_client(
            config,
            state.db.clone(),
            client,
            Some(private_client),
            private_credentials,
            state.realtime.clone(),
        )
        .await?;
    Ok(code_ok(serde_json::to_value(status)?))
}

fn live_param_overrides(req: &LocalApiRequest) -> AppResult<Map<String, Value>> {
    let params = match req.body.get("params").or_else(|| req.params.get("params")) {
        None => Map::new(),
        Some(Value::Object(params)) => params.clone(),
        Some(_) => {
            return Err(AppError::Validation(
                "实时策略参数 params 必须是 JSON 对象".to_string(),
            ));
        }
    };
    if params.contains_key("portfolio_layers") {
        return Err(AppError::Validation(
            "实时策略已删除 portfolio_layers 本地组合架构，请不要在启动参数中传入 portfolio_layers"
                .to_string(),
        ));
    }
    Ok(params)
}

fn has_explicit_mode(req: &LocalApiRequest) -> bool {
    req.body
        .get("mode")
        .or_else(|| req.params.get("mode"))
        .is_some()
}

pub(in crate::commands::local_api) async fn stop_live_strategy(
    state: &AppState,
) -> AppResult<Value> {
    let status = state.live_strategy.stop().await;
    Ok(code_ok(serde_json::to_value(status)?))
}

pub(in crate::commands::local_api) async fn live_strategy_orders(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 50).clamp(1, 500);
    let mode = if has_explicit_mode(req) {
        super::request_live_strategy_mode(req, "simulated")?
    } else {
        request_trading_mode(state, req).await?
    };
    let run_id = param_string(req, "run_id", "");
    let orders = crate::live_strategy::query_live_orders(&state.db, limit, &mode, &run_id).await?;
    Ok(code_ok(Value::Array(orders)))
}

pub(in crate::commands::local_api) async fn live_execution_plans(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 50).clamp(1, 500);
    let mode = if has_explicit_mode(req) {
        super::request_live_strategy_mode(req, "simulated")?
    } else {
        request_trading_mode(state, req).await?
    };
    let run_id = param_string(req, "run_id", "");
    let plans =
        crate::live_strategy::query_live_execution_plans(&state.db, limit, &mode, &run_id).await?;
    Ok(code_ok(Value::Array(plans)))
}

pub(in crate::commands::local_api) async fn live_strategy_equity(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = if has_explicit_mode(req) {
        super::request_live_strategy_mode(req, "simulated")?
    } else {
        request_trading_mode(state, req).await?
    };
    let run_id = param_string(req, "run_id", "");
    let status = state.live_strategy.status().await;
    if !run_id.trim().is_empty()
        && (!status.mode.eq_ignore_ascii_case(&mode) || status.run_id != run_id)
    {
        return Ok(code_ok(empty_okx_account_equity_history_payload(
            &mode,
            &run_id,
            "okx_account_balance_unscoped",
        )));
    }
    let (items, source) = if let Some(cached_items) = state
        .realtime
        .latest_private_account_balance_items(&mode)
        .await
    {
        (cached_items, "okx_account_ws_cache")
    } else {
        let client = okx_private_client(state, &mode).await?;
        (client.get_account_balance().await?, "okx_account_balance")
    };
    Ok(code_ok(okx_account_equity_history_payload(
        &items,
        &mode,
        &run_id,
        &status,
        chrono::Utc::now().timestamp_millis(),
        source,
    )?))
}

fn okx_account_equity_history_payload(
    items: &[Value],
    mode: &str,
    run_id: &str,
    status: &crate::live_strategy::LiveStrategyStatus,
    emitted_at_ms: i64,
    source: &str,
) -> AppResult<Value> {
    let account = account_equity_snapshot(items, emitted_at_ms).ok_or_else(|| {
        AppError::Validation("OKX 账户余额未返回有效权益，无法生成实时策略权益快照".to_string())
    })?;
    let scoped_run_id = if run_id.trim().is_empty()
        && status.mode.eq_ignore_ascii_case(mode)
        && !status.run_id.trim().is_empty()
    {
        status.run_id.clone()
    } else {
        run_id.trim().to_string()
    };
    let status_matches_scope = !scoped_run_id.is_empty()
        && status.mode.eq_ignore_ascii_case(mode)
        && status.run_id == scoped_run_id;
    let strategy_id = if status_matches_scope {
        status.strategy_id.clone()
    } else {
        String::new()
    };
    let strategy_name = if status_matches_scope {
        status.strategy_name.clone()
    } else {
        String::new()
    };
    let symbol = if status_matches_scope {
        status.symbol.clone()
    } else {
        String::new()
    };
    let timeframe = status_matches_scope
        .then(|| status.timeframe.clone())
        .filter(|item| !item.trim().is_empty())
        .unwrap_or_else(|| "1H".to_string());
    let inst_type = status_matches_scope
        .then(|| status.inst_type.clone())
        .filter(|item| !item.trim().is_empty())
        .unwrap_or_else(|| "SPOT".to_string());
    let initial_capital = if status_matches_scope
        && status.initial_capital.is_finite()
        && status.initial_capital > 0.0
    {
        status.initial_capital
    } else {
        account.equity
    };
    let timestamp = account.timestamp_ms;
    let timestamp_text = timestamp_iso(timestamp);
    let trading_day = crate::risk_controls::trading_day(timestamp);
    let snapshot = serde_json::json!({
        "id": 0,
        "run_id": scoped_run_id,
        "strategy_id": strategy_id,
        "strategy_name": strategy_name,
        "symbol": symbol,
        "inst_id": symbol,
        "timeframe": timeframe,
        "inst_type": inst_type,
        "mode": mode,
        "timestamp": timestamp,
        "time": timestamp_text,
        "trading_day": trading_day,
        "price": null,
        "position_side": null,
        "entry_price": null,
        "quantity": null,
        "initial_capital": initial_capital,
        "day_start_equity": account.equity,
        "equity": account.equity,
        "realized_pnl": null,
        "unrealized_pnl": account.unrealized_pnl,
        "total_pnl": null,
        "total_pnl_pct": null,
        "today_pnl": null,
        "today_pnl_pct": null,
        "created_at": timestamp_text,
        "pnl_available": false,
        "source": source
    });
    let daily = serde_json::json!({
        "trading_day": trading_day,
        "start_timestamp": timestamp,
        "end_timestamp": timestamp,
        "start_time": timestamp_text,
        "end_time": timestamp_text,
        "snapshot_count": 1,
        "first_equity": account.equity,
        "last_equity": account.equity,
        "day_start_equity": account.equity,
        "today_pnl": null,
        "today_pnl_pct": null,
        "total_pnl": null,
        "total_pnl_pct": null,
        "realized_pnl": null,
        "unrealized_pnl": account.unrealized_pnl,
        "pnl_available": false,
    });
    Ok(serde_json::json!({
        "mode": mode,
        "run_id": scoped_run_id,
        "count": 1,
        "snapshots": [snapshot],
        "daily": [daily],
        "pnl_available": false,
        "source": source
    }))
}

fn empty_okx_account_equity_history_payload(mode: &str, run_id: &str, source: &str) -> Value {
    serde_json::json!({
        "mode": mode,
        "run_id": run_id,
        "count": 0,
        "snapshots": [],
        "daily": [],
        "source": source
    })
}

#[derive(Clone, Copy, Debug)]
struct OkxAccountEquitySnapshot {
    equity: f64,
    unrealized_pnl: Option<f64>,
    timestamp_ms: i64,
}

fn account_equity_snapshot(
    items: &[Value],
    emitted_at_ms: i64,
) -> Option<OkxAccountEquitySnapshot> {
    let account = items.first()?;
    let details = account
        .get("details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let equity = finite_value(account, "totalEq")
        .or_else(|| finite_value(account, "adjEq"))
        .or_else(|| sum_first_finite_detail_value(&details, &["eqUsd", "disEq", "eq"]))?;
    let unrealized_pnl = sum_detail_value(&details, "upl");
    let cached_received_ms =
        integer_value(account, "_okxq_received_at_ms").filter(|value| *value > 0);
    let timestamp_ms = if cached_received_ms.is_some() {
        latest_account_update_ms(account, &details)
            .or(cached_received_ms)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
    } else {
        (emitted_at_ms > 0)
            .then_some(emitted_at_ms)
            .or_else(|| latest_account_update_ms(account, &details))
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
    };
    Some(OkxAccountEquitySnapshot {
        equity,
        unrealized_pnl,
        timestamp_ms,
    })
}

fn sum_first_finite_detail_value(details: &[Value], keys: &[&str]) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for detail in details {
        if let Some(value) = keys.iter().find_map(|key| finite_value(detail, key)) {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

fn sum_detail_value(details: &[Value], key: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for detail in details {
        if let Some(value) = finite_value(detail, key) {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

fn latest_account_update_ms(account: &Value, details: &[Value]) -> Option<i64> {
    let account_ts = integer_value(account, "uTime").filter(|value| *value > 0);
    details
        .iter()
        .filter_map(|detail| integer_value(detail, "uTime").filter(|value| *value > 0))
        .chain(account_ts)
        .max()
}

fn finite_value(value: &Value, key: &str) -> Option<f64> {
    let parsed = match value.get(key)? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    parsed.is_finite().then_some(parsed)
}

fn integer_value(value: &Value, key: &str) -> Option<i64> {
    match value.get(key)? {
        Value::Number(item) => item
            .as_i64()
            .or_else(|| item.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(item) => item.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn timestamp_iso(timestamp: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use serde_json::Map;

    use super::*;

    #[test]
    fn live_param_overrides_reject_portfolio_layers_instead_of_stripping() {
        let req = test_request(
            json!({
                "params": {
                    "symbol": "ETH-USDT-SWAP",
                    "timeframe": "15m",
                    "leverage": 5,
                    "portfolio_layers": [{"symbol": "BTC-USDT-SWAP"}]
                }
            }),
            Map::new(),
        );

        let error = live_param_overrides(&req).expect_err("portfolio_layers must be rejected");

        assert!(error.to_string().contains("portfolio_layers 本地组合架构"));
    }

    #[test]
    fn live_param_overrides_keep_supported_run_params() {
        let req = test_request(
            json!({
                "params": {
                    "symbol": "ETH-USDT-SWAP",
                    "timeframe": "15m",
                    "leverage": 5
                }
            }),
            Map::new(),
        );

        let overrides = live_param_overrides(&req).expect("params object should parse");

        assert_eq!(overrides.get("symbol"), Some(&json!("ETH-USDT-SWAP")));
        assert_eq!(overrides.get("timeframe"), Some(&json!("15m")));
        assert_eq!(overrides.get("leverage"), Some(&json!(5)));
        assert_eq!(
            request_runtime_string(&req, &overrides, "symbol", "BTC-USDT-SWAP"),
            "ETH-USDT-SWAP"
        );
        assert_eq!(request_runtime_f64(&req, &overrides, "leverage", 1.0), 5.0);
    }

    #[test]
    fn top_level_live_runtime_values_override_nested_params() {
        let req = test_request(
            json!({
                "symbol": "SOL-USDT-SWAP",
                "check_interval": "7",
                "params": {
                    "symbol": "ETH-USDT-SWAP",
                    "check_interval": 60
                }
            }),
            Map::new(),
        );
        let overrides = live_param_overrides(&req).expect("params object should parse");

        assert_eq!(
            request_runtime_string(&req, &overrides, "symbol", "BTC-USDT-SWAP"),
            "SOL-USDT-SWAP"
        );
        assert_eq!(
            request_runtime_f64(&req, &overrides, "check_interval", 60.0),
            7.0
        );
    }

    #[test]
    fn live_param_overrides_reject_non_object_params() {
        let req = test_request(json!({"params": ["bad"]}), Map::new());

        let error = live_param_overrides(&req).expect_err("params must be object");

        assert!(error.to_string().contains("params 必须是 JSON 对象"));
    }

    #[test]
    fn live_start_mode_rejects_removed_paper_alias() {
        let req = test_request(json!({"mode": "paper"}), Map::new());

        let error = super::super::request_live_strategy_mode(&req, "simulated")
            .expect_err("paper must not be normalized");

        assert!(error.to_string().contains("只支持 live 或 simulated"));
    }

    #[test]
    fn live_start_mode_rejects_non_string_mode() {
        let req = test_request(json!({"mode": 1}), Map::new());

        let error = super::super::request_live_strategy_mode(&req, "simulated")
            .expect_err("numeric mode must be rejected");

        assert!(error.to_string().contains("mode 必须是字符串"));
    }

    #[test]
    fn live_start_mode_rejects_removed_demo_and_simulation_aliases() {
        for mode in ["demo", "simulation"] {
            let req = test_request(json!({"mode": mode}), Map::new());

            let error = super::super::request_live_strategy_mode(&req, "live")
                .expect_err("live mode aliases must not be normalized");

            assert!(
                error.to_string().contains("只支持 live 或 simulated"),
                "unexpected error for mode={mode}: {error}"
            );
        }
    }

    #[test]
    fn okx_account_equity_history_uses_account_balance_snapshot() {
        let status = crate::live_strategy::LiveStrategyStatus {
            status: "running".to_string(),
            run_id: "run-okx-equity".to_string(),
            mode: "simulated".to_string(),
            strategy_id: "strategy_a".to_string(),
            strategy_name: "Strategy A".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            inst_type: "SWAP".to_string(),
            initial_capital: 1_000.0,
            ..crate::live_strategy::LiveStrategyStatus::default()
        };
        let payload = okx_account_equity_history_payload(
            &[json!({
                "totalEq": "1234.56",
                "details": [
                    {"ccy": "USDT", "eqUsd": "1000", "upl": "12.5", "uTime": "1780000000000"},
                    {"ccy": "BTC", "eqUsd": "234.56", "upl": "-2.5", "uTime": "1780000060000"}
                ]
            })],
            "simulated",
            "run-okx-equity",
            &status,
            1_780_000_123_456,
            "okx_account_balance",
        )
        .expect("OKX account balance should build equity history");

        assert_eq!(payload["source"], "okx_account_balance");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["run_id"], "run-okx-equity");
        assert_eq!(payload["snapshots"][0]["equity"], 1234.56);
        assert_eq!(payload["snapshots"][0]["unrealized_pnl"], 10.0);
        assert!(payload["snapshots"][0]["position_side"].is_null());
        assert_eq!(payload["snapshots"][0]["pnl_available"], false);
        assert_eq!(payload["pnl_available"], false);
        assert_eq!(payload["snapshots"][0]["timestamp"], 1_780_000_123_456_i64);
        assert_eq!(payload["daily"][0]["last_equity"], 1234.56);
    }

    #[test]
    fn okx_account_equity_history_does_not_fabricate_strategy_pnl_or_position_fields() {
        let payload = okx_account_equity_history_payload(
            &[json!({
                "totalEq": "1234.56",
                "details": [
                    {"ccy": "USDT", "eqUsd": "1234.56", "upl": "10", "uTime": "1780000000000"}
                ]
            })],
            "simulated",
            "",
            &crate::live_strategy::LiveStrategyStatus::default(),
            1_780_000_123_456,
            "okx_account_ws_cache",
        )
        .expect("OKX account balance should build equity history");

        assert_eq!(payload["source"], "okx_account_ws_cache");
        assert_eq!(payload["snapshots"][0]["source"], "okx_account_ws_cache");
        let snapshot = &payload["snapshots"][0];
        assert!(snapshot["price"].is_null());
        assert!(snapshot["position_side"].is_null());
        assert!(snapshot["entry_price"].is_null());
        assert!(snapshot["quantity"].is_null());
        assert!(snapshot["realized_pnl"].is_null());
        assert!(snapshot["total_pnl"].is_null());
        assert!(snapshot["total_pnl_pct"].is_null());
        assert!(snapshot["today_pnl"].is_null());
        assert!(snapshot["today_pnl_pct"].is_null());
        assert_eq!(snapshot["unrealized_pnl"], 10.0);

        let daily = &payload["daily"][0];
        assert!(daily["today_pnl"].is_null());
        assert!(daily["today_pnl_pct"].is_null());
        assert!(daily["total_pnl"].is_null());
        assert!(daily["total_pnl_pct"].is_null());
        assert!(daily["realized_pnl"].is_null());
        assert_eq!(daily["unrealized_pnl"], 10.0);
    }

    #[test]
    fn okx_account_equity_history_sums_details_when_total_equity_is_missing() {
        let payload = okx_account_equity_history_payload(
            &[json!({
                "details": [
                    {"ccy": "USDT", "eqUsd": "900", "uTime": "1780000000000"},
                    {"ccy": "ETH", "disEq": "120.5", "uTime": "1780000001000"}
                ]
            })],
            "live",
            "",
            &crate::live_strategy::LiveStrategyStatus::default(),
            1_780_000_123_456,
            "okx_account_balance",
        )
        .expect("detail equity should build account snapshot");

        assert_eq!(payload["source"], "okx_account_balance");
        assert_eq!(payload["run_id"], "");
        assert_eq!(payload["snapshots"][0]["equity"], 1020.5);
        assert!(payload["snapshots"][0]["unrealized_pnl"].is_null());
        assert_eq!(payload["snapshots"][0]["timestamp"], 1_780_000_123_456_i64);
    }

    #[test]
    fn cached_okx_account_equity_history_uses_cached_event_time_not_poll_time() {
        let payload = okx_account_equity_history_payload(
            &[json!({
                "totalEq": "1234.56",
                "_okxq_received_at_ms": 1_780_000_010_000_i64,
                "details": [
                    {"ccy": "USDT", "eqUsd": "1234.56"}
                ]
            })],
            "simulated",
            "",
            &crate::live_strategy::LiveStrategyStatus::default(),
            1_780_000_123_456,
            "okx_account_ws_cache",
        )
        .expect("cached account balance should build equity history");

        assert_eq!(payload["snapshots"][0]["timestamp"], 1_780_000_010_000_i64);
    }

    #[test]
    fn okx_account_equity_history_rejects_invalid_equity_numbers() {
        let error = okx_account_equity_history_payload(
            &[json!({
                "totalEq": "not-a-number",
                "details": [
                    {"ccy": "USDT", "eqUsd": "bad", "disEq": null, "eq": {}}
                ]
            })],
            "simulated",
            "",
            &crate::live_strategy::LiveStrategyStatus::default(),
            1_780_000_123_456,
            "okx_account_balance",
        )
        .expect_err("invalid OKX equity must not be converted to a zero snapshot");

        assert!(error.to_string().contains("有效权益"));
    }

    fn test_request(
        body: serde_json::Value,
        params: Map<String, serde_json::Value>,
    ) -> LocalApiRequest {
        LocalApiRequest {
            method: "POST".to_string(),
            path: "/api/live/start".to_string(),
            params,
            body,
        }
    }
}
