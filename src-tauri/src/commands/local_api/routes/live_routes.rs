use serde_json::Value;

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};

use super::{
    super::{backtest, code_ok, live, LocalApiRequest},
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
        ("GET", ["api", "live", "available-strategies"]) => {
            Ok(code_ok(backtest::available_strategies(state).await?))
        }
        ("GET", ["api", "live", "status"]) => live::live_strategy_status(state).await,
        ("GET", ["api", "live", "execution-logs"]) => live::live_execution_logs(state, req).await,
        ("GET", ["api", "live", "orders"]) => live::live_strategy_orders(state, req).await,
        ("GET", ["api", "live", "execution-plans"]) => live::live_execution_plans(state, req).await,
        ("GET", ["api", "live", "equity"]) => live::live_strategy_equity(state, req).await,
        ("POST", ["api", "live", "decision-diagnostics"]) => {
            live::live_decision_diagnostics(state, req).await
        }
        ("POST", ["api", "live", "start"]) => live::start_live_strategy(state, req).await,
        ("POST", ["api", "live", "stop"]) => live::stop_live_strategy(state).await,
        ("POST", ["api", "live", ..]) => {
            Err(AppError::Validation("不支持的实时策略操作".to_string()))
        }
        _ => unsupported_route(method, path),
    }
}
