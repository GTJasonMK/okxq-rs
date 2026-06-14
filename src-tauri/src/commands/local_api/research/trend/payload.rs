use serde_json::{json, Value};

use crate::{
    app_state::AppState, commands::local_api::now_unix_seconds, config::TrendResearchConfig,
};

pub(super) fn trend_settings_payload(cfg: &TrendResearchConfig) -> Value {
    json!({
        "enabled": cfg.enabled,
        "whitelist": cfg.whitelist.clone(),
        "feature_bar_seconds": cfg.feature_bar_seconds,
        "state_sync_seconds": cfg.state_sync_seconds,
        "book_channel": cfg.book_channel.clone()
    })
}

pub(super) fn trend_default_settings_payload() -> Value {
    let defaults = TrendResearchConfig::default();
    trend_settings_payload(&defaults)
}

pub(super) fn trend_status(
    enabled: bool,
    whitelist: &[String],
    inference_count: usize,
) -> &'static str {
    if !enabled {
        "disabled"
    } else if whitelist.is_empty() {
        "unconfigured"
    } else if inference_count == 0 {
        "collecting"
    } else {
        "ready"
    }
}

pub(super) fn trend_model_status() -> Value {
    json!({
        "status": "ready",
        "model_ref": "rust-local-baseline",
        "updated_at": now_unix_seconds()
    })
}

pub(super) async fn trend_meta_payload(state: &AppState, inference_count: usize) -> Value {
    let cfg = state.config.read().await;
    let status = trend_status(
        cfg.trend_research.enabled,
        &cfg.trend_research.whitelist,
        inference_count,
    );
    json!({
        "enabled": cfg.trend_research.enabled,
        "status": status,
        "whitelist": cfg.trend_research.whitelist.clone(),
        "runtime_error": "",
        "model_status": trend_model_status()
    })
}
