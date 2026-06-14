use serde_json::{Map, Value};

use crate::{
    live_strategy::LiveStrategyConfig,
    okx::OkxCandle,
    strategy_engine::StrategyConfig,
    strategy_executor::{self, types::RuntimeStrategyMeta},
};

use super::{
    cache::{
        decision_cache_get, decision_cache_set, live_computation_cache_key,
        runtime_strategy_source_stamp, CachedDiagnosticDecision,
    },
    context::{build_live_strategy_context, LiveStrategyContextInput},
    params::{diagnostic_candle_count, strategy_config_json},
    response::enrich_strategy_output,
};
use crate::commands::local_api::helpers::okx_private_client;
use crate::commands::local_api::live::{
    candles::{candle_to_json, load_latest_diagnostic_candles, merge_latest_diagnostic_candle},
    execution::{
        attach_execution_decision, execution_decision_requires_exchange_risk_state,
        ExchangeRiskEvidence,
    },
    request_live_strategy_mode,
};
use crate::commands::local_api::*;

struct DiagnosticStrategyInput {
    runtime_meta: RuntimeStrategyMeta,
    config: StrategyConfig,
}

pub(in crate::commands::local_api) async fn live_decision_diagnostics(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let DiagnosticStrategyInput {
        runtime_meta,
        config,
    } = diagnostic_strategy_input(state, req)?;
    let default_mode = state.config.read().await.okx.default_mode().to_string();
    let mode = request_live_strategy_mode(req, &default_mode)?;
    let (risk_enabled, max_single_loss, max_position_pct, max_order_value) =
        load_risk_control_for_live(state, &mode).await;
    let limit = diagnostic_candle_limit(req, &config, 0);
    let fresh = body_bool(req, "fresh", false);
    let (candles, realtime_candle_applied) =
        load_diagnostic_candles(state, req, &config, limit, fresh, "无法诊断决策").await?;
    let live_config = LiveStrategyConfig {
        strategy_id: config.strategy_id.clone(),
        strategy_name: config.strategy_name.clone(),
        symbol: config.symbol.clone(),
        timeframe: config.timeframe.clone(),
        inst_type: config.inst_type.clone(),
        mode,
        initial_capital: config.initial_capital,
        position_size: config.position_size,
        stop_loss: config.stop_loss,
        take_profit: config.take_profit,
        risk_timeframe: body_string(req, "risk_timeframe", "1m"),
        check_interval: body_i64(req, "check_interval", 60).max(1) as u64,
        params: config.params.clone(),
        project_root: state.paths.root.clone(),
        risk_control_enabled: risk_enabled,
        max_single_loss_ratio: max_single_loss,
        max_position_pct,
        max_order_value,
    };

    let runtime_status = state.live_strategy.status().await;
    let config_json = strategy_config_json(&config);
    let candles_json = candles.iter().map(candle_to_json).collect::<Vec<Value>>();
    let context = build_live_strategy_context(LiveStrategyContextInput {
        state,
        req,
        runtime_meta: &runtime_meta,
        config: &config,
        mode: &live_config.mode,
        primary_candles: &candles,
        primary_candles_json: &candles_json,
        limit,
        fresh,
        runtime_status: &runtime_status,
    })
    .await?;
    let context_stamp = strategy_executor::context_cache_stamp(&context);
    let cache_key = live_computation_cache_key(
        &config,
        &candles,
        &runtime_strategy_source_stamp(&state.paths.root, &runtime_meta.file_name),
        Some(&context_stamp),
    );
    let decision_output = if let Some(cached) = decision_cache_get(&cache_key) {
        cached
    } else {
        let decision = strategy_executor::compute_runtime_diagnostics_decision_with_context(
            &state.paths.root,
            &runtime_meta.file_name,
            &config_json,
            &candles_json,
            &context,
        )?;
        let diagnostics = strategy_executor::runtime_diagnostics_response_from_decision(&decision);
        let decision_output = CachedDiagnosticDecision {
            actions: decision.actions,
            diagnostics,
        };
        decision_cache_set(cache_key, &decision_output);
        decision_output
    };

    let exchange_risk_evidence =
        execution_decision_exchange_risk_evidence(state, &live_config).await;
    let mut diagnostics = enrich_strategy_output(
        decision_output.diagnostics,
        &config,
        candles.len(),
        realtime_candle_applied,
    );
    attach_execution_decision(
        &mut diagnostics,
        &decision_output.actions,
        &live_config,
        &runtime_status,
        candles
            .last()
            .ok_or_else(|| AppError::Validation("诊断 K 线为空，无法预览执行".to_string()))?,
        exchange_risk_evidence.as_ref(),
    );
    Ok(code_ok(diagnostics))
}

fn diagnostic_strategy_input(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<DiagnosticStrategyInput> {
    let strategy_id = body_string(req, "strategy_id", "");
    let runtime_meta = strategy_executor::ensure_registered(&state.paths.root, &strategy_id)?;
    let raw_symbol = body_string(req, "symbol", "BTC-USDT-SWAP");
    let inferred_inst_type = infer_inst_type(&raw_symbol);
    let inst_type = body_string(req, "inst_type", &inferred_inst_type).to_uppercase();
    let symbol = backtest::normalize_strategy_inst_id(&raw_symbol, &inst_type)?;
    let timeframe = request_timeframe(req)?;
    let raw_params = diagnostic_context_params(req)?;
    let params = strategy_executor::merge_default_params(&strategy_id, raw_params);
    Ok(DiagnosticStrategyInput {
        config: StrategyConfig {
            strategy_id,
            strategy_name: runtime_meta.strategy_name.clone(),
            symbol,
            inst_type,
            timeframe,
            initial_capital: request_f64(req, "initial_capital", 10_000.0),
            position_size: request_f64(req, "position_size", 0.2),
            stop_loss: request_f64(req, "stop_loss", 0.05),
            take_profit: request_f64(req, "take_profit", 0.10),
            params: Value::Object(params),
        },
        runtime_meta,
    })
}

fn diagnostic_candle_limit(
    req: &LocalApiRequest,
    config: &StrategyConfig,
    minimum_auto_limit: usize,
) -> usize {
    let requested_limit = body_i64(req, "limit", 0);
    if requested_limit > 0 {
        return (requested_limit as usize).clamp(3, 20_000);
    }
    diagnostic_candle_count(&config.params, &config.timeframe)
        .max(minimum_auto_limit)
        .clamp(3, 20_000)
}

async fn load_diagnostic_candles(
    state: &AppState,
    req: &LocalApiRequest,
    config: &StrategyConfig,
    limit: usize,
    fresh: bool,
    insufficient_context: &str,
) -> AppResult<(Vec<OkxCandle>, bool)> {
    market_ops::ensure_local_candles_for_read(
        state,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        limit as i64,
        fresh,
    )
    .await?;
    let mut candles = load_latest_diagnostic_candles(
        state,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        limit as i64,
    )
    .await?;
    let realtime_candle_applied = merge_latest_diagnostic_candle(
        req,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        limit,
        &mut candles,
    )?;
    if candles.len() < 3 {
        return Err(AppError::Validation(format!(
            "{} {} {} K 线数量不足，{}",
            config.symbol, config.inst_type, config.timeframe, insufficient_context
        )));
    }
    Ok((candles, realtime_candle_applied))
}

fn diagnostic_context_params(req: &LocalApiRequest) -> AppResult<Map<String, Value>> {
    let params = match req.body.get("params").or_else(|| req.params.get("params")) {
        None => Map::new(),
        Some(Value::Object(params)) => params.clone(),
        Some(_) => {
            return Err(AppError::Validation(
                "实时策略诊断参数 params 必须是 JSON 对象".to_string(),
            ));
        }
    };
    if params.contains_key("portfolio_layers") {
        return Err(AppError::Validation(
            "实时策略已删除 portfolio_layers 本地组合架构，请不要在诊断参数中传入 portfolio_layers"
                .to_string(),
        ));
    }
    Ok(params)
}

async fn execution_decision_exchange_risk_evidence(
    state: &AppState,
    config: &LiveStrategyConfig,
) -> Option<ExchangeRiskEvidence> {
    if !execution_decision_requires_exchange_risk_state(config) {
        return None;
    }
    let client = match okx_private_client(state, &config.mode).await {
        Ok(client) => client,
        Err(error) => {
            return Some(ExchangeRiskEvidence::Error(format!(
                "获取 OKX 风控状态失败: {error}"
            )));
        }
    };
    match crate::live_strategy::LiveStrategyRuntime::exchange_runtime_risk_state(config, &client)
        .await
    {
        Ok(_) => Some(ExchangeRiskEvidence::State),
        Err(error) => Some(ExchangeRiskEvidence::Error(format!(
            "获取 OKX 风控状态失败: {error}"
        ))),
    }
}

fn request_timeframe(req: &LocalApiRequest) -> AppResult<String> {
    let raw = body_string(req, "timeframe", "15m");
    let normalized = market_ops::normalize_timeframe_name(&raw);
    if normalized.is_empty() {
        return Err(AppError::Validation(format!(
            "不支持的 K 线周期: {}，当前仅支持 1m/3m/5m/15m/30m/1H/2H/4H/6H/12H/1D/1W/1M",
            raw.trim()
        )));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn diagnostic_candle_limit_keeps_explicit_limit_authoritative() {
        let req = request(json!({ "limit": 9 }));

        assert_eq!(
            diagnostic_candle_limit(&req, &strategy_config(json!({})), 620),
            9
        );
    }

    #[test]
    fn diagnostic_candle_limit_keeps_auto_window_above_minimum() {
        let req = request(json!({}));

        assert_eq!(
            diagnostic_candle_limit(&req, &strategy_config(json!({})), 620),
            620
        );
    }

    #[test]
    fn diagnostic_candle_limit_uses_runtime_action_context_window_rules() {
        let req = request(json!({}));

        assert_eq!(
            diagnostic_candle_limit(
                &req,
                &strategy_config(json!({
                    "vol_window_15m": 300,
                    "threshold_lookback_15m": 20
                })),
                0
            ),
            321
        );
    }

    #[test]
    fn diagnostic_context_params_rejects_portfolio_layers() {
        let req = request(json!({
            "params": {
                "symbol": "ETH-USDT-SWAP",
                "portfolio_layers": [{"symbol": "BTC-USDT-SWAP"}]
            }
        }));

        let error = diagnostic_context_params(&req).expect_err("portfolio_layers must be rejected");

        assert!(error.to_string().contains("portfolio_layers 本地组合架构"));
    }

    #[test]
    fn diagnostic_context_params_rejects_non_object_params() {
        let req = request(json!({ "params": ["bad"] }));

        let error = diagnostic_context_params(&req).expect_err("params must be object");

        assert!(error.to_string().contains("params 必须是 JSON 对象"));
    }

    #[test]
    fn diagnostic_context_params_keeps_supported_params() {
        let req = request(json!({
            "params": {
                "symbol": "ETH-USDT-SWAP",
                "timeframe": "15m",
                "leverage": 5
            }
        }));

        let params = diagnostic_context_params(&req).expect("params object should parse");

        assert_eq!(params.get("symbol"), Some(&json!("ETH-USDT-SWAP")));
        assert_eq!(params.get("timeframe"), Some(&json!("15m")));
        assert_eq!(params.get("leverage"), Some(&json!(5)));
    }

    fn request(body: Value) -> LocalApiRequest {
        LocalApiRequest {
            method: "POST".to_string(),
            path: "/api/live/decision-diagnostics".to_string(),
            params: serde_json::Map::new(),
            body,
        }
    }

    fn strategy_config(params: Value) -> StrategyConfig {
        StrategyConfig {
            strategy_id: "test".to_string(),
            strategy_name: "Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "15m".to_string(),
            initial_capital: 10_000.0,
            position_size: 0.2,
            stop_loss: 0.05,
            take_profit: 0.10,
            params,
        }
    }
}
