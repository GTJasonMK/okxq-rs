use super::{
    super::*,
    payload::{
        trend_default_settings_payload, trend_model_status, trend_settings_payload, trend_status,
    },
};

pub(crate) async fn trend_research_config(state: &AppState) -> AppResult<Value> {
    let cfg = state.config.read().await;
    let meta = trend_status(cfg.trend_research.enabled, &cfg.trend_research.whitelist, 0);
    Ok(json!({
        "settings": trend_settings_payload(&cfg.trend_research),
        "defaults": trend_default_settings_payload(),
        "enabled": cfg.trend_research.enabled,
        "status": meta,
        "whitelist": cfg.trend_research.whitelist.clone(),
        "runtime_error": "",
        "model_status": trend_model_status()
    }))
}

pub(crate) async fn update_trend_research_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let whitelist_supplied =
        req.body.get("whitelist").is_some() || req.params.get("whitelist").is_some();
    let validated_whitelist = if whitelist_supplied {
        let requested_inst_type = request_string(req, "inst_type", "");
        let mut items = Vec::new();
        for item in request_string_array(req, "whitelist") {
            if item.trim().is_empty() {
                continue;
            }
            let (inst_id, _) = resolve_research_scope(state, &item, &requested_inst_type).await?;
            items.push(inst_id);
        }
        items.sort();
        items.dedup();
        Some(items)
    } else {
        None
    };

    let mut cfg = state.config.write().await;
    cfg.trend_research.enabled = request_bool(req, "enabled", cfg.trend_research.enabled);
    if let Some(whitelist) = validated_whitelist {
        cfg.trend_research.whitelist = whitelist;
    }
    cfg.trend_research.feature_bar_seconds = request_i64(
        req,
        "feature_bar_seconds",
        cfg.trend_research.feature_bar_seconds as i64,
    )
    .clamp(1, 60) as u64;
    cfg.trend_research.state_sync_seconds = request_i64(
        req,
        "state_sync_seconds",
        cfg.trend_research.state_sync_seconds as i64,
    )
    .clamp(5, 3600) as u64;
    cfg.trend_research.book_channel =
        request_string(req, "book_channel", &cfg.trend_research.book_channel);
    state
        .config_manager
        .save_trend_research(&cfg.trend_research)?;
    let settings = trend_settings_payload(&cfg.trend_research);
    let status = trend_status(cfg.trend_research.enabled, &cfg.trend_research.whitelist, 0);
    Ok(json!({
        "settings": settings,
        "enabled": cfg.trend_research.enabled,
        "status": status,
        "whitelist": cfg.trend_research.whitelist.clone(),
        "runtime_error": "",
        "model_status": trend_model_status()
    }))
}
