use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::config::TrendResearchConfig;

use super::*;

/// GET /api/market/tick-collector/status
pub(super) async fn tick_collector_status(state: &AppState) -> AppResult<Value> {
    Ok(code_ok(serde_json::to_value(
        state.tick_collector.status().await,
    )?))
}

/// POST /api/market/tick-collector/start
pub(super) async fn start_tick_collector(state: &AppState) -> AppResult<Value> {
    let cfg = tick_collector_config(state).await?;
    match state.tick_collector.start(state.db.clone(), cfg).await {
        Ok(tx) => {
            state.realtime.set_tick_collector_tx(tx).await;
            let status = state.tick_collector.status().await;
            let feed_status = state
                .realtime
                .subscribe_collection_feeds(&status.active_symbols, &status.book_channel)
                .await?;
            Ok(code_ok(json!({
                "message": "秒级数据采集器已启动，成交与盘口订阅已建立",
                "status": status,
                "realtime": feed_status
            })))
        }
        Err(error) => Err(AppError::Runtime(error)),
    }
}

/// POST /api/market/tick-collector/stop
pub(super) async fn stop_tick_collector(state: &AppState) -> AppResult<Value> {
    let before = state.tick_collector.status().await;
    let _ = state
        .realtime
        .unsubscribe_collection_feeds(&before.active_symbols, &before.book_channel)
        .await;
    state.realtime.clear_tick_collector_tx().await;
    let status = state.tick_collector.stop().await;
    Ok(code_ok(
        json!({"message": "秒级数据采集器已停止", "status": status}),
    ))
}

async fn tick_collector_config(state: &AppState) -> AppResult<TrendResearchConfig> {
    let mut cfg = {
        let guard = state.config.read().await;
        guard.trend_research.clone()
    };
    if !cfg.whitelist.is_empty() {
        return Ok(cfg);
    }

    let mut symbols = BTreeSet::new();
    for record in state.preferences.watched_symbols().await? {
        if record.sync_swap {
            let inst_id = record.swap_inst_id.trim().to_uppercase();
            if !inst_id.is_empty() {
                symbols.insert(inst_id);
            }
        } else if record.sync_spot {
            let inst_id = record.spot_inst_id.trim().to_uppercase();
            if !inst_id.is_empty() {
                symbols.insert(inst_id);
            }
        }
    }
    if !symbols.is_empty() {
        cfg.whitelist = symbols.into_iter().collect();
    }
    Ok(cfg)
}
