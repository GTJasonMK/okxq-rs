use serde_json::{json, Value};

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{backtest, code_ok, LocalApiRequest},
    unsupported_route,
};

pub(super) async fn dispatch(
    state: &AppState,
    req: &LocalApiRequest,
    method: &str,
    path: &str,
    segment_refs: &[&str],
) -> AppResult<Value> {
    match (method, segment_refs) {
        ("GET", ["api", "backtest", "strategies"]) => {
            Ok(code_ok(backtest::available_strategies(state).await?))
        }
        ("POST", ["api", "backtest", "strategies", "reload"]) => {
            let strategies = backtest::available_strategies(state).await?;
            Ok(code_ok(json!({
                "total": strategies.as_array().map(|items| items.len()).unwrap_or(0),
                "strategies": strategies,
                "runtime_loaded": true
            })))
        }
        ("GET", ["api", "backtest", "history"]) => backtest::backtest_history(state, req).await,
        ("GET", ["api", "backtest", "history", result_id]) => {
            backtest::backtest_detail(state, result_id).await
        }
        ("GET", ["api", "backtest", "progress", run_id]) => {
            backtest::backtest_progress(state, run_id).await
        }
        ("DELETE", ["api", "backtest", "history", result_id]) => {
            backtest::delete_backtest_result(state, result_id).await
        }
        ("POST", ["api", "backtest", "run", strategy_id]) => {
            backtest::run_backtest_strategy(state, strategy_id, req).await
        }
        ("POST", ["api", "backtest", "monte-carlo", result_id]) => {
            backtest::run_monte_carlo_analysis(state, result_id, req).await
        }
        ("POST", ["api", "backtest", "walk-forward", result_id]) => {
            backtest::run_walk_forward_analysis(state, result_id, req).await
        }
        ("POST", ["api", "backtest", ..]) => Ok(json!({
            "code": 404,
            "message": "不支持的回测路由",
            "data": null
        })),
        _ => unsupported_route(method, path),
    }
}
