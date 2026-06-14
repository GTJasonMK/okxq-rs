use std::collections::BTreeSet;

use crate::sync_jobs::SyncJob;

use super::super::*;

pub(in crate::commands::local_api) fn request_inst_id(req: &LocalApiRequest) -> String {
    let inst_id = request_string(req, "inst_id", "");
    if inst_id.is_empty() {
        request_string(req, "instId", "")
    } else {
        inst_id
    }
}

pub(in crate::commands::local_api) async fn ensure_realtime_inst_allowed(
    state: &AppState,
    inst_id: &str,
) -> AppResult<()> {
    let inst_id = inst_id.trim().to_uppercase();
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let scopes = watched_instrument_scopes(state).await?;
    let inst_type = infer_inst_type(&inst_id);
    if watched_request_allowed(&scopes, &inst_id, &inst_type) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "{inst_id} {inst_type} 未在关注清单中启用，已拒绝实时订阅"
    )))
}

pub(in crate::commands::local_api) async fn ensure_timeframe_allowed_for_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<()> {
    ensure_timeframes_allowed_for_scope(
        state,
        inst_id,
        inst_type,
        &[normalize_timeframe_name(timeframe)],
    )
    .await
}

pub(in crate::commands::local_api) async fn watched_instrument_scopes(
    state: &AppState,
) -> AppResult<BTreeSet<(String, String)>> {
    let watched = state.preferences.watched_symbols().await?;
    let mut scopes = BTreeSet::new();
    for record in watched {
        if record.sync_spot {
            scopes.insert((
                record.spot_inst_id.trim().to_uppercase(),
                "SPOT".to_string(),
            ));
        }
        if record.sync_swap {
            scopes.insert((
                record.swap_inst_id.trim().to_uppercase(),
                "SWAP".to_string(),
            ));
        }
    }
    Ok(scopes)
}

pub(in crate::commands::local_api) fn watched_job_allowed(
    scopes: &BTreeSet<(String, String)>,
    job: &SyncJob,
) -> bool {
    watched_request_allowed(scopes, &job.inst_id, &job.inst_type)
}

pub(in crate::commands::local_api) fn watched_request_allowed(
    scopes: &BTreeSet<(String, String)>,
    inst_id: &str,
    inst_type: &str,
) -> bool {
    let normalized_inst_id = inst_id.trim().to_uppercase();
    let normalized_inst_type = inst_type.trim().to_uppercase();
    scopes.contains(&(normalized_inst_id, normalized_inst_type))
}

pub(in crate::commands::local_api) async fn ensure_timeframes_allowed_for_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframes: &[String],
) -> AppResult<()> {
    let normalized_inst_id = inst_id.trim().to_uppercase();
    let normalized_inst_type = inst_type.trim().to_uppercase();
    let requested = timeframes
        .iter()
        .map(|timeframe| normalize_timeframe_name(timeframe))
        .filter(|timeframe| !timeframe.is_empty())
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Err(AppError::Validation(
            "至少需要一个有效 K 线周期".to_string(),
        ));
    }

    let settings = guardian_settings(state).await?;
    let watched = state.preferences.watched_symbols().await?;
    let Some(record) = watched.into_iter().find(|record| {
        (normalized_inst_type == "SPOT"
            && record.sync_spot
            && record
                .spot_inst_id
                .eq_ignore_ascii_case(&normalized_inst_id))
            || (normalized_inst_type == "SWAP"
                && record.sync_swap
                && record
                    .swap_inst_id
                    .eq_ignore_ascii_case(&normalized_inst_id))
    }) else {
        return Err(AppError::Validation(format!(
            "{} {} 未在关注清单中启用，已拒绝读取或同步 K 线",
            normalized_inst_id, normalized_inst_type
        )));
    };

    let plans = watch_sync_plans_for_record(&record, &settings);
    let allowed = target_timeframes_from_plans(&plans)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let denied = requested
        .into_iter()
        .filter(|timeframe| !allowed.contains(timeframe))
        .collect::<Vec<_>>();
    if denied.is_empty() {
        return Ok(());
    }

    Err(AppError::Validation(format!(
        "{} {} 未在关注规则中启用周期 {}，已拒绝读取或同步 K 线",
        normalized_inst_id,
        normalized_inst_type,
        denied.join("/")
    )))
}
