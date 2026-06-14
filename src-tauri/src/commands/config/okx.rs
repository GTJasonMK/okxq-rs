use serde_json::json;
use tauri::State;

use crate::{
    app_state::AppState,
    error::AppResult,
    okx_network::{effective_proxy_label, OKX_BUSINESS_WS_URL, OKX_PUBLIC_WS_URL},
};

use self::{
    connection_test::test_okx_config,
    mask::mask_okx_config,
    merge::merge_okx_config,
    types::{OkxConfigRequest, OkxConfigResponse},
};

mod connection_test;
mod mask;
mod merge;
mod types;

#[tauri::command]
pub async fn get_okx_config(state: State<'_, AppState>) -> AppResult<OkxConfigResponse> {
    let cfg = state.config.read().await;
    tracing::debug!(
        mode = cfg.okx.default_mode(),
        demo_configured = cfg.okx.demo.is_valid(),
        live_configured = cfg.okx.live.is_valid(),
        "OKX configuration requested"
    );
    Ok(mask_okx_config(&cfg.okx))
}

#[tauri::command]
pub async fn save_okx_config(
    state: State<'_, AppState>,
    req: OkxConfigRequest,
) -> AppResult<serde_json::Value> {
    let next = {
        let mut cfg = state.config.write().await;
        let next = merge_okx_config(Some(&req), &cfg.okx)?;
        state.config_manager.save_okx(&next)?;
        cfg.okx = next.clone();
        next
    };

    state.realtime.set_proxy_url(next.proxy_url.clone()).await;
    tracing::info!(
        mode = next.default_mode(),
        demo_configured = next.demo.is_valid(),
        live_configured = next.live.is_valid(),
        proxy = effective_proxy_label(&next.proxy_url).as_str(),
        "OKX configuration saved"
    );

    Ok(json!({
        "success": true,
        "message": format!(
            "配置已保存并生效（当前模式: {}）",
            if next.use_simulated { "模拟盘" } else { "实盘" }
        )
    }))
}

#[tauri::command]
pub async fn test_okx_connection(
    state: State<'_, AppState>,
    req: Option<OkxConfigRequest>,
) -> AppResult<serde_json::Value> {
    let okx = {
        let cfg = state.config.read().await;
        merge_okx_config(req.as_ref(), &cfg.okx)?
    };
    test_okx_config(&state, okx, OKX_PUBLIC_WS_URL, OKX_BUSINESS_WS_URL).await
}
