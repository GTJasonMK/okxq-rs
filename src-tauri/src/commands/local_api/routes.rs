use serde_json::Value;

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};

use super::LocalApiRequest;

mod agent_routes;
mod assistant_routes;
mod backtest_routes;
mod journal_routes;
mod live_routes;
mod market_routes;
mod research_routes;
mod scanner_routes;
mod system_routes;
mod trading_routes;

pub(super) async fn dispatch_local_api_request(
    state: &AppState,
    req: &LocalApiRequest,
    method: &str,
    path: &str,
    segment_refs: &[&str],
) -> AppResult<Value> {
    match segment_refs {
        ["health"] | ["status"] | ["api", "system", ..] => {
            system_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "market", ..] => {
            market_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "backtest", ..] => {
            backtest_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "journal", ..] => {
            journal_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "scanner", ..] => {
            scanner_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "risk", ..] | ["api", "trading", ..] => {
            trading_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "live", ..] => live_routes::dispatch(state, req, method, path, segment_refs).await,
        ["api", "assistant", ..] => {
            assistant_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "agent", ..] => {
            agent_routes::dispatch(state, req, method, path, segment_refs).await
        }
        ["api", "data-center", ..]
        | ["api", "research-platform", ..]
        | ["api", "trend-research", ..]
        | ["api", "research", ..] => {
            research_routes::dispatch(state, req, method, path, segment_refs).await
        }
        _ => unsupported_route(method, path),
    }
}

pub(super) fn unsupported_route(method: &str, path: &str) -> AppResult<Value> {
    Err(AppError::Validation(format!(
        "Unsupported local API route: {} {}",
        method, path
    )))
}
