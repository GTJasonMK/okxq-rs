use serde::Deserialize;
use serde_json::json;
use tauri::State;

use crate::{
    app_state::AppState,
    config::{mask_key, sanitize_secret_value, AssistantConfig},
    error::AppResult,
};

#[derive(Debug, Deserialize)]
pub struct AssistantConfigRequest {
    pub enabled: bool,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub provider_name: Option<String>,
}

#[tauri::command]
pub async fn get_assistant_config(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    let cfg = state.config.read().await;
    tracing::debug!(
        enabled = cfg.assistant.enabled,
        configured = cfg.assistant.is_configured(),
        provider = cfg.assistant.provider_name.as_str(),
        model = cfg.assistant.model.as_str(),
        "assistant configuration requested"
    );
    Ok(json!({
        "enabled": cfg.assistant.enabled,
        "configured": cfg.assistant.is_configured(),
        "base_url": cfg.assistant.base_url,
        "api_key": mask_key(&cfg.assistant.api_key),
        "model": cfg.assistant.model,
        "provider_name": cfg.assistant.provider_name
    }))
}

#[tauri::command]
pub async fn save_assistant_config(
    state: State<'_, AppState>,
    req: AssistantConfigRequest,
) -> AppResult<serde_json::Value> {
    let mut cfg = state.config.write().await;
    let api_key = sanitize_secret_value(
        "assistant.api_key",
        req.api_key.as_deref().unwrap_or_default(),
        &cfg.assistant.api_key,
    )?;
    let next = AssistantConfig {
        enabled: req.enabled,
        base_url: non_empty_or(req.base_url, "https://api.openai.com/v1"),
        api_key,
        model: non_empty_or(req.model, "gpt-4.1-mini"),
        provider_name: non_empty_or(req.provider_name, "OpenAI"),
    };

    state.config_manager.save_assistant(&next)?;
    cfg.assistant = next;
    tracing::info!(
        enabled = cfg.assistant.enabled,
        configured = cfg.assistant.is_configured(),
        provider = cfg.assistant.provider_name.as_str(),
        model = cfg.assistant.model.as_str(),
        "assistant configuration saved"
    );

    Ok(json!({
        "success": true,
        "message": "AI 助手配置已保存并生效"
    }))
}

fn non_empty_or(value: Option<String>, default_value: &str) -> String {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}
