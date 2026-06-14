use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{agent, LocalApiRequest},
    error::AppResult,
};

pub(in crate::commands::local_api::assistant) async fn execute_tool(
    state: &AppState,
    tool_name: &str,
    arguments: &Value,
) -> AppResult<String> {
    let tool_req = LocalApiRequest {
        method: "POST".to_string(),
        path: format!("/api/agent/query/{tool_name}"),
        params: Default::default(),
        body: arguments.clone(),
    };
    match tool_name {
        "get_market_snapshot" => {
            let result = agent::query_market_snapshot(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_candles" => {
            let result = agent::query_candles(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_indicators" => {
            let result = agent::query_indicators(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_trading_context" => {
            let result = agent::query_trading_context(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "scan_watchlist" => {
            let result = agent::query_watchlist_scan(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_orderbook" => {
            let result = agent::query_orderbook(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_recent_trades" => {
            let result = agent::query_recent_trades(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "get_position" => {
            let result = agent::query_position(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "check_data_health" => {
            let result = agent::query_data_health(state, &tool_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "analyze_multi_timeframe" => {
            let analysis_req = assistant_analysis_request(
                "/api/agent/analysis/multi-timeframe-alignment",
                arguments,
            );
            let result = agent::analyze_multi_timeframe_alignment(state, &analysis_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "analyze_correlation" => {
            let analysis_req =
                assistant_analysis_request("/api/agent/analysis/watchlist-correlation", arguments);
            let result = agent::analyze_watchlist_correlation(state, &analysis_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "patrol_opportunities" => {
            let analysis_req =
                assistant_analysis_request("/api/agent/analysis/opportunity-patrol", arguments);
            let result = agent::analyze_opportunity_patrol(state, &analysis_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        "calculate_risk_budget" => {
            let analysis_req =
                assistant_analysis_request("/api/agent/analysis/risk-budget", arguments);
            let result = agent::analyze_risk_budget(state, &analysis_req).await?;
            Ok(serde_json::to_string(&result)?)
        }
        _ => Ok(json!({"error": format!("未知工具: {tool_name}")}).to_string()),
    }
}

fn assistant_analysis_request(path: &str, arguments: &Value) -> LocalApiRequest {
    LocalApiRequest {
        method: "POST".to_string(),
        path: path.to_string(),
        params: Default::default(),
        body: arguments.clone(),
    }
}
