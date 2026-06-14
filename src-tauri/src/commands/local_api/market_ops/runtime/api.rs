use serde_json::{json, Map, Value};

use super::super::*;
use super::limiter::sync_job_limiter;
use super::settings::SyncRuntimeSettings;
use super::state::{
    load_sync_runtime_settings, store_sync_runtime_settings, SYNC_RUNTIME_SETTINGS_KEY,
};

pub(in crate::commands::local_api) async fn sync_runtime_config(
    state: &AppState,
) -> AppResult<Value> {
    let settings = load_sync_runtime_settings(state).await?;
    Ok(code_ok(json!({
        "settings": settings,
        "defaults": SyncRuntimeSettings::default(),
        "limits": SyncRuntimeSettings::limits(),
        "active_sync_jobs": sync_job_limiter().active(),
        "message": "采集性能参数保存后对后续同步任务生效"
    })))
}

pub(in crate::commands::local_api) async fn update_sync_runtime_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let raw = req
        .body
        .get("settings")
        .cloned()
        .unwrap_or_else(|| req.body.clone());
    let settings = SyncRuntimeSettings::from_value(raw);
    let mut partial = Map::new();
    partial.insert(
        SYNC_RUNTIME_SETTINGS_KEY.to_string(),
        serde_json::to_value(&settings)?,
    );
    state.preferences.merge(partial).await?;
    store_sync_runtime_settings(&settings);
    state
        .token_bucket
        .apply_concurrency_settings(&settings.okx_concurrency_settings());
    sync_job_limiter().notify_limit_change();
    Ok(code_ok(json!({
        "settings": settings,
        "defaults": SyncRuntimeSettings::default(),
        "limits": SyncRuntimeSettings::limits(),
        "active_sync_jobs": sync_job_limiter().active(),
        "message": "采集性能参数已保存，后续同步任务将使用新配置"
    })))
}
