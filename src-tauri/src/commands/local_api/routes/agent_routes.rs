use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{agent, code_ok, LocalApiRequest},
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
        ("GET", ["api", "agent", "capabilities"]) => Ok(code_ok(agent::agent_capabilities())),
        ("POST", ["api", "agent", "query", "market-snapshot"]) => {
            agent::query_market_snapshot(state, req).await
        }
        ("POST", ["api", "agent", "query", "candles"]) => agent::query_candles(state, req).await,
        ("POST", ["api", "agent", "query", "indicators"]) => {
            agent::query_indicators(state, req).await
        }
        ("POST", ["api", "agent", "query", "trading-context"]) => {
            agent::query_trading_context(state, req).await
        }
        ("POST", ["api", "agent", "query", "watchlist-scan"]) => {
            agent::query_watchlist_scan(state, req).await
        }
        ("POST", ["api", "agent", "query", "data-health"]) => {
            agent::query_data_health(state, req).await
        }
        ("POST", ["api", "agent", "query", "orderbook"]) => {
            agent::query_orderbook(state, req).await
        }
        ("POST", ["api", "agent", "query", "recent-trades"]) => {
            agent::query_recent_trades(state, req).await
        }
        ("POST", ["api", "agent", "query", "position"]) => agent::query_position(state, req).await,
        ("POST", ["api", "agent", "analysis", "multi-timeframe-alignment"]) => {
            agent::analyze_multi_timeframe_alignment(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "watchlist-correlation"]) => {
            agent::analyze_watchlist_correlation(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "opportunity-patrol"]) => {
            agent::analyze_opportunity_patrol(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "market-structure"]) => {
            agent::analyze_market_structure(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "support-resistance"]) => {
            agent::analyze_support_resistance(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "price-projection"]) => {
            agent::analyze_price_projection(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "trade-setup"]) => {
            agent::analyze_trade_setup(state, req).await
        }
        ("POST", ["api", "agent", "analysis", "risk-budget"]) => {
            agent::analyze_risk_budget(state, req).await
        }
        ("POST", ["api", "agent", "analyze", "python"]) => agent::analyze_python(state, req).await,
        _ => unsupported_route(method, path),
    }
}
