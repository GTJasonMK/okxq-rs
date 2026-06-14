use serde_json::{Map, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        request_runtime_f64 as runtime_request_f64, request_runtime_string,
        resolve_enabled_market_scope, LocalApiRequest,
    },
    error::{AppError, AppResult},
    market_candle_rows::{
        load_recent_valid_candle_rows_until, load_valid_candle_rows_in_range, okx_candles_from_rows,
    },
    okx::OkxCandle,
    strategy_engine::StrategyConfig,
    strategy_executor,
};

use super::window::{expected_backtest_candle_count, BacktestWindow, MAX_BACKTEST_CANDLES};

pub(super) async fn backtest_strategy_config(
    state: &AppState,
    strategy_id: &str,
    req: &LocalApiRequest,
) -> AppResult<StrategyConfig> {
    let meta = strategy_executor::ensure_registered(&state.paths.root, strategy_id)?;
    let runtime = &meta.runtime_config;
    let param_overrides = request_param_overrides(req)?;
    let raw_symbol = request_runtime_string(
        req,
        &param_overrides,
        "symbol",
        &runtime_string(runtime, "symbol", "BTC-USDT"),
    );
    let raw_inst_type = request_runtime_string(
        req,
        &param_overrides,
        "inst_type",
        &runtime_string(runtime, "inst_type", ""),
    );
    let (symbol, inst_type) =
        resolve_enabled_market_scope(state, &raw_symbol, &raw_inst_type).await?;
    let params = strategy_executor::merge_default_params(strategy_id, param_overrides.clone());
    Ok(StrategyConfig {
        strategy_id: strategy_id.to_string(),
        strategy_name: meta.strategy_name,
        symbol,
        inst_type,
        timeframe: request_runtime_string(
            req,
            &param_overrides,
            "timeframe",
            &runtime_string(runtime, "timeframe", "1H"),
        ),
        initial_capital: request_positive_runtime_f64(
            req,
            &param_overrides,
            "initial_capital",
            runtime_f64(runtime, "initial_capital", 10_000.0),
        )?,
        position_size: runtime_request_f64(
            req,
            &param_overrides,
            "position_size",
            runtime_f64(runtime, "position_size", 0.2),
        ),
        stop_loss: runtime_request_f64(
            req,
            &param_overrides,
            "stop_loss",
            runtime_f64(runtime, "stop_loss", 0.05),
        ),
        take_profit: runtime_request_f64(
            req,
            &param_overrides,
            "take_profit",
            runtime_f64(runtime, "take_profit", 0.10),
        ),
        params: Value::Object(params),
    })
}

fn request_positive_runtime_f64(
    req: &LocalApiRequest,
    params: &Map<String, Value>,
    key: &str,
    fallback: f64,
) -> AppResult<f64> {
    let explicit = req
        .body
        .get(key)
        .or_else(|| req.params.get(key))
        .or_else(|| params.get(key));
    let value = if let Some(explicit) = explicit {
        crate::commands::local_api::finite_value_from_json(explicit)
            .ok_or_else(|| AppError::Validation(format!("回测参数 {key} 必须是大于 0 的数字")))?
    } else {
        fallback
    };
    if value.is_finite() && value > 0.0 {
        return Ok(value);
    }
    Err(AppError::Validation(format!(
        "回测参数 {key} 必须是大于 0 的数字"
    )))
}

fn request_param_overrides(req: &LocalApiRequest) -> AppResult<Map<String, Value>> {
    match req.body.get("params").or_else(|| req.params.get("params")) {
        None => Ok(Map::new()),
        Some(Value::Object(params)) => Ok(params.clone()),
        Some(_) => Err(AppError::Validation(
            "回测参数 params 必须是 JSON 对象".to_string(),
        )),
    }
}

pub(in crate::commands::local_api) fn runtime_string(
    runtime: &Value,
    key: &str,
    default: &str,
) -> String {
    runtime
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default)
        .to_string()
}

pub(in crate::commands::local_api) fn runtime_f64(runtime: &Value, key: &str, default: f64) -> f64 {
    runtime
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

pub(in crate::commands::local_api) fn normalize_strategy_inst_id(
    raw: &str,
    inst_type: &str,
) -> AppResult<String> {
    strategy_executor::normalize_runtime_inst_id(raw, inst_type)
}

pub(super) async fn load_backtest_candles(
    state: &AppState,
    config: &StrategyConfig,
    window: BacktestWindow,
) -> AppResult<Vec<OkxCandle>> {
    let expected_candles = expected_backtest_candle_count(&config.timeframe, window);
    let local_candles = load_backtest_candles_from_db(&state.db, config, window).await?;
    if local_candles.len() >= expected_candles as usize {
        return Ok(local_candles);
    }

    super::super::market_ops::ensure_local_candles_range_coverage(
        state,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        window.start_ts,
        window.end_ts,
    )
    .await?;
    load_backtest_candles_from_db(&state.db, config, window).await
}

pub(super) async fn load_backtest_candles_from_db(
    db: &sqlx::SqlitePool,
    config: &StrategyConfig,
    window: BacktestWindow,
) -> AppResult<Vec<OkxCandle>> {
    let rows = load_valid_candle_rows_in_range(
        db,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        window.start_ts,
        window.end_ts,
        Some(MAX_BACKTEST_CANDLES),
    )
    .await?;
    let candles = okx_candles_from_rows(rows, "1");
    if candles.len() < 20 {
        return Err(AppError::Validation(format!(
            "{} {} 在所选区间可用 K 线不足，请先在数据中心同步历史行情或调整日期范围",
            config.symbol, config.timeframe
        )));
    }
    Ok(candles)
}

pub(super) async fn load_backtest_context_candles_from_db(
    db: &sqlx::SqlitePool,
    config: &StrategyConfig,
    window: BacktestWindow,
    min_bars: usize,
) -> AppResult<Vec<OkxCandle>> {
    let window_bars = expected_backtest_candle_count(&config.timeframe, window);
    let limit = (min_bars as i64)
        .saturating_add(window_bars)
        .clamp(3, MAX_BACKTEST_CANDLES);
    let rows = load_recent_valid_candle_rows_until(
        db,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        window.end_ts,
        limit,
    )
    .await?;
    Ok(okx_candles_from_rows(rows, "1"))
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::*;

    fn request(body: Value) -> LocalApiRequest {
        LocalApiRequest {
            method: "POST".to_string(),
            path: "/api/backtest/run/test".to_string(),
            params: Map::new(),
            body,
        }
    }

    #[test]
    fn request_param_overrides_requires_object_params() {
        let req = request(json!({
            "params": {
                "leverage": 3,
                "strict_context_gating": false
            }
        }));

        let params = request_param_overrides(&req).expect("params");

        assert_eq!(params.get("leverage").and_then(Value::as_i64), Some(3));
        assert_eq!(
            params.get("strict_context_gating").and_then(Value::as_bool),
            Some(false)
        );
        assert!(request_param_overrides(&request(json!({ "params": [] }))).is_err());
    }

    #[test]
    fn runtime_numeric_overrides_parse_strings_like_live() {
        let params = request_param_overrides(&request(json!({
            "params": {
                "position_size": "0.35",
                "stop_loss": "0.02"
            }
        })))
        .expect("params");

        let empty_req = request(json!({}));
        assert_eq!(
            crate::commands::local_api::request_runtime_f64(
                &empty_req,
                &params,
                "position_size",
                0.2
            ),
            0.35
        );
        assert_eq!(
            crate::commands::local_api::request_runtime_f64(&empty_req, &params, "stop_loss", 0.05),
            0.02
        );
        assert_eq!(
            crate::commands::local_api::request_runtime_f64(
                &empty_req,
                &params,
                "take_profit",
                0.10
            ),
            0.10
        );
    }

    #[test]
    fn request_runtime_f64_prefers_top_level_then_params_override() {
        let req = request(json!({
            "initial_capital": 250.0,
            "params": {
                "initial_capital": 100.0
            }
        }));
        let params = request_param_overrides(&req).expect("params");

        assert_eq!(
            request_positive_runtime_f64(&req, &params, "initial_capital", 10_000.0).unwrap(),
            250.0
        );

        let req = request(json!({
            "params": {
                "initial_capital": 100.0
            }
        }));
        let params = request_param_overrides(&req).expect("params");
        assert_eq!(
            request_positive_runtime_f64(&req, &params, "initial_capital", 10_000.0).unwrap(),
            100.0
        );
    }

    #[test]
    fn request_runtime_string_prefers_top_level_then_params_override() {
        let req = request(json!({
            "symbol": "SOL-USDT-SWAP",
            "params": {
                "symbol": "ETH-USDT-SWAP",
                "timeframe": "15m"
            }
        }));
        let params = request_param_overrides(&req).expect("params");

        assert_eq!(
            crate::commands::local_api::request_runtime_string(
                &req,
                &params,
                "symbol",
                "BTC-USDT-SWAP"
            ),
            "SOL-USDT-SWAP"
        );
        assert_eq!(
            crate::commands::local_api::request_runtime_string(&req, &params, "timeframe", "1H"),
            "15m"
        );
    }

    #[test]
    fn request_runtime_f64_rejects_invalid_initial_capital() {
        let req = request(json!({
            "initial_capital": 0.0
        }));
        let params = request_param_overrides(&req).expect("params");

        let error = request_positive_runtime_f64(&req, &params, "initial_capital", 10_000.0)
            .unwrap_err()
            .to_string();

        assert!(error.contains("initial_capital 必须是大于 0 的数字"));

        let req = request(json!({
            "initial_capital": "100"
        }));
        let params = request_param_overrides(&req).expect("params");
        assert_eq!(
            request_positive_runtime_f64(&req, &params, "initial_capital", 10_000.0).unwrap(),
            100.0
        );

        let req = request(json!({
            "initial_capital": "bad"
        }));
        let params = request_param_overrides(&req).expect("params");
        let error = request_positive_runtime_f64(&req, &params, "initial_capital", 10_000.0)
            .unwrap_err()
            .to_string();

        assert!(error.contains("initial_capital 必须是大于 0 的数字"));
    }
}
