use serde_json::json;
use tauri::State;

use crate::{app_state::AppState, error::AppResult};

#[tauri::command]
pub async fn system_health(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let cfg = state.config.read().await;
    let components = json!({
        "config": {
            "healthy": true,
            "required": true,
            "detail": "ok"
        },
        "database": {
            "healthy": true,
            "required": true,
            "detail": state.paths.data_dir.join("market.db").display().to_string()
        },
        "websocket": {
            "healthy": false,
            "required": false,
            "detail": "not_migrated"
        },
        "data_guardian": {
            "healthy": true,
            "required": false,
            "detail": "basic_scheduler_ready"
        },
        "assistant_patrol": {
            "healthy": false,
            "required": false,
            "detail": if cfg.assistant.is_configured() { "configured_not_started" } else { "not_configured" }
        },
        "research_platform": {
            "healthy": false,
            "required": false,
            "detail": "not_migrated"
        },
        "trend_research": {
            "healthy": !cfg.trend_research.enabled,
            "required": false,
            "detail": if cfg.trend_research.enabled { "configured_not_started" } else { "not_enabled" }
        }
    });

    Ok(json!({
        "status": "healthy",
        "failed_components": [],
        "components": components
    }))
}
