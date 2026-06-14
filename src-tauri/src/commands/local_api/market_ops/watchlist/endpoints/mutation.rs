use crate::config::{AddWatchedSymbolRequest, WatchedSymbolSyncPlan};

use super::super::{super::*, jobs::*};

pub(in crate::commands::local_api) async fn add_watch_symbol(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let symbol = body_string(req, "symbol", "");
    let normalized = normalize_symbol(&symbol)
        .ok_or_else(|| AppError::Validation("币种不能为空".to_string()))?;
    let previous = state
        .preferences
        .watched_symbols()
        .await?
        .into_iter()
        .find(|item| item.symbol == normalized);
    let sync_spot = req.body.get("sync_spot").and_then(Value::as_bool);
    let sync_swap = req.body.get("sync_swap").and_then(Value::as_bool);
    let archive_all_history = req.body.get("archive_all_history").and_then(Value::as_bool);
    let sync_days = req.body.get("sync_days").and_then(Value::as_i64);
    let sync_plans = req
        .body
        .get("sync_plans")
        .map(|value| {
            serde_json::from_value::<Vec<WatchedSymbolSyncPlan>>(value.clone())
                .map_err(|error| AppError::Validation(format!("K 线采集规则格式无效: {error}")))
        })
        .transpose()?;
    let auto_sync = req
        .body
        .get("auto_sync")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let payload = AddWatchedSymbolRequest {
        symbol,
        sync_spot,
        sync_swap,
        archive_all_history,
        sync_days,
        sync_plans,
    };
    let result = state.preferences.add_watched_symbol(payload).await?;

    let cancelled_disabled_jobs =
        cancel_jobs_for_disabled_watch_markets(state, previous.as_ref(), &result.watched_symbol)
            .await?;
    let sync_policy_changed = previous.as_ref().is_some_and(|item| {
        item.archive_all_history != result.watched_symbol.archive_all_history
            || item.sync_days != result.watched_symbol.sync_days
            || item.sync_plans != result.watched_symbol.sync_plans
    });
    let should_sync_spot = if result.existed {
        result.watched_symbol.sync_spot
            && (!previous.as_ref().is_some_and(|item| item.sync_spot) || sync_policy_changed)
    } else {
        result.watched_symbol.sync_spot
    };
    let should_sync_swap = if result.existed {
        result.watched_symbol.sync_swap
            && (!previous.as_ref().is_some_and(|item| item.sync_swap) || sync_policy_changed)
    } else {
        result.watched_symbol.sync_swap
    };
    let (sync_jobs, reused_count, exact_gap_jobs, rule_jobs) = if auto_sync {
        let repair_jobs = enqueue_watched_record_repair_jobs(
            state,
            &result.watched_symbol,
            sync_spot.unwrap_or(true) && should_sync_spot,
            sync_swap.unwrap_or(true) && should_sync_swap,
        )
        .await?;
        (
            repair_jobs.sync_jobs,
            repair_jobs.reused_count,
            repair_jobs.exact_gap_jobs,
            repair_jobs.rule_jobs,
        )
    } else {
        (Vec::new(), 0, 0, 0)
    };
    let started_count = sync_jobs.len().saturating_sub(reused_count);
    Ok(code_ok(json!({
        "watched_symbol": result.watched_symbol,
        "existed": result.existed,
        "started_count": started_count,
        "reused_count": reused_count,
        "exact_gap_jobs": exact_gap_jobs,
        "rule_jobs": rule_jobs,
        "cancelled_disabled_jobs": cancelled_disabled_jobs,
        "sync_jobs": sync_jobs
    })))
}

pub(in crate::commands::local_api) async fn delete_watched_symbol(
    state: &AppState,
    symbol: &str,
) -> AppResult<Value> {
    let normalized =
        normalize_symbol(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let removed = state.preferences.remove_watched_symbol(&normalized).await?;
    if removed.is_none() {
        return Ok(json!({
            "code": 404,
            "message": "关注币种不存在",
            "data": null
        }));
    }
    mark_inventory_deletion_requested(&state.db, &normalized, "watched_symbol_delete").await?;
    let active_sync_jobs = cancel_related_sync_jobs(
        state,
        &normalized,
        "币种已从关注列表移除，后台同步任务已取消",
    )
    .await?;
    let counts =
        delete_marked_symbol_related_data(&state.db, &normalized, "watched_symbol_delete").await?;
    Ok(code_ok(json!({
        "symbol": normalized,
        "deleted": true,
        "deleted_counts": counts,
        "active_sync_jobs": active_sync_jobs
    })))
}

pub(in crate::commands::local_api) async fn repair_watched_symbol(
    state: &AppState,
    symbol: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let normalized =
        normalize_symbol(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let records = state.preferences.watched_symbols().await?;
    let Some(record) = records.into_iter().find(|item| item.symbol == normalized) else {
        return Ok(json!({"code": 404, "message": "关注币种不存在", "data": null}));
    };
    let requested_spot = request_bool(req, "sync_spot", true);
    let requested_swap = request_bool(req, "sync_swap", true);
    if !requested_spot && !requested_swap {
        return Err(AppError::Validation(
            "至少需要选择现货或永续中的一种修复目标".to_string(),
        ));
    }
    let effective_spot = requested_spot && record.sync_spot;
    let effective_swap = requested_swap && record.sync_swap;
    if !effective_spot && !effective_swap {
        return Err(AppError::Validation(
            "该币当前未启用所选市场的同步配置".to_string(),
        ));
    }
    let repair_jobs =
        enqueue_watched_record_repair_jobs(state, &record, effective_spot, effective_swap).await?;
    let started_count = repair_jobs
        .sync_jobs
        .len()
        .saturating_sub(repair_jobs.reused_count);
    Ok(code_ok(json!({
        "symbol": record.symbol,
        "sync_jobs": repair_jobs.sync_jobs,
        "requested_markets": {"spot": requested_spot, "swap": requested_swap},
        "effective_markets": {
            "spot": effective_spot,
            "swap": effective_swap
        },
        "started_count": started_count,
        "reused_count": repair_jobs.reused_count,
        "exact_gap_jobs": repair_jobs.exact_gap_jobs,
        "rule_jobs": repair_jobs.rule_jobs,
        "message": "Rust OKX 公共行情回补已执行"
    })))
}
