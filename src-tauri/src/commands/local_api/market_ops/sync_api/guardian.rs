use serde_json::{json, Map, Value};

use crate::{
    guardian::{GuardianSettings, GUARDIAN_SETTINGS_KEY},
    sync_jobs::SyncJobRequest,
};

use super::super::{watchlist::enqueue_watched_record_gap_repair_jobs, *};

pub(in crate::commands::local_api) async fn guardian_config(state: &AppState) -> AppResult<Value> {
    Ok(code_ok(json!({
        "settings": guardian_settings(state).await?,
        "defaults": GuardianSettings::default()
    })))
}

pub(in crate::commands::local_api) async fn update_guardian_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let settings = serde_json::from_value::<GuardianSettings>(req.body.clone())
        .map_err(|error| AppError::Validation(format!("数据守护配置格式无效: {error}")))?
        .normalized();
    let mut partial = Map::new();
    partial.insert(
        GUARDIAN_SETTINGS_KEY.to_string(),
        serde_json::to_value(&settings)?,
    );
    state.preferences.merge(partial).await?;
    Ok(code_ok(json!({
        "settings": settings,
        "defaults": GuardianSettings::default(),
        "status": guardian_status_payload(state).await?
    })))
}

pub(in crate::commands::local_api) async fn guardian_status(state: &AppState) -> AppResult<Value> {
    Ok(code_ok(guardian_status_payload(state).await?))
}

pub(in crate::commands::local_api) async fn run_guardian_now(state: &AppState) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let settings = guardian_settings(state).await?;
    let watched = state.preferences.watched_symbols().await?;

    let mut requests = Vec::<SyncJobRequest>::new();
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut full_remaining = settings.max_full_backfill_jobs_per_cycle;
    for record in watched {
        let needs_rule_sync = match enqueue_watched_record_gap_repair_jobs(
            state,
            &record,
            &settings,
            record.sync_spot,
            record.sync_swap,
        )
        .await
        {
            Ok((mut gap_jobs, _, needs_rule_sync)) => {
                results.append(&mut gap_jobs);
                needs_rule_sync
            }
            Err(error) => {
                errors.push(json!({
                    "scope": "schedule_gap_repair_job",
                    "symbol": record.symbol,
                    "message": error.to_string()
                }));
                continue;
            }
        };

        if !needs_rule_sync {
            continue;
        }

        match watched_record_sync_requests(
            state,
            &record,
            &settings,
            record.sync_spot,
            record.sync_swap,
            false,
        )
        .await
        {
            Ok(rule_requests) => {
                for request in rule_requests {
                    let mode = request.mode.clone();
                    if mode == "full" {
                        if full_remaining <= 0 {
                            continue;
                        }
                        full_remaining -= 1;
                    }
                    requests.push(request);
                }
            }
            Err(error) => {
                errors.push(json!({
                    "scope": "plan_rule_sync_job",
                    "symbol": record.symbol,
                    "message": error.to_string()
                }));
            }
        }
    }

    state
        .guardian
        .start_cycle((results.len() + requests.len()) as i64)
        .await;
    for (index, value) in results.iter().enumerate() {
        state
            .guardian
            .record_scheduled(
                value.get("inst_id").and_then(Value::as_str).unwrap_or(""),
                value.get("timeframe").and_then(Value::as_str).unwrap_or(""),
                value
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("gap_repair"),
                (index + 1) as i64,
            )
            .await;
    }
    let scheduled_gap_jobs = results.len();
    for (index, request) in requests.into_iter().enumerate() {
        state
            .guardian
            .record_scheduled(
                &request.inst_id,
                &request.timeframe,
                &request.mode,
                (scheduled_gap_jobs + index + 1) as i64,
            )
            .await;
        match enqueue_sync_job(state, request).await {
            Ok((job, reused)) => {
                results.push(submitted_sync_job_value(&job, reused));
            }
            Err(error) => {
                errors.push(json!({
                    "scope": "schedule_sync_job",
                    "message": error.to_string()
                }));
            }
        }
    }
    state.guardian.finish_cycle(results, errors).await;
    Ok(code_ok(guardian_status_payload(state).await?))
}
