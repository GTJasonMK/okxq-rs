use serde_json::{json, Value};

use super::super::*;

pub(in crate::commands::local_api) async fn sync_candles_job(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let request = sync_request_from_api(req)?;
    ensure_sync_request_allowed(state, &request).await?;
    let data = run_sync_request_guarded(state, request).await?;
    Ok(code_ok(data))
}

pub(in crate::commands::local_api) async fn start_sync_job(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let request = sync_request_from_api(req)?;
    ensure_sync_request_allowed(state, &request).await?;
    let (job, reused) = enqueue_sync_job(state, request).await?;
    let mut data = job.to_value()?;
    if let Some(obj) = data.as_object_mut() {
        obj.insert("reused_existing".to_string(), Value::Bool(reused));
    }
    Ok(code_ok(data))
}

pub(in crate::commands::local_api) async fn sync_jobs(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let active_only = param_bool(req, "active_only", false);
    let watched_only = param_bool(req, "watched_only", false);
    let watched_scopes = if watched_only {
        Some(watched_instrument_scopes(state).await?)
    } else {
        None
    };
    let limit = param_i64(req, "limit", 20).clamp(1, 200) as usize;
    let task_ids = request_string_array(req, "task_ids");
    let jobs = state
        .sync_jobs
        .list(
            active_only,
            limit,
            if task_ids.is_empty() {
                None
            } else {
                Some(task_ids.as_slice())
            },
        )
        .await?
        .into_iter()
        .filter(|job| {
            watched_scopes
                .as_ref()
                .map(|scopes| watched_job_allowed(scopes, job))
                .unwrap_or(true)
        })
        .map(|job| job.to_value())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(code_ok(Value::Array(jobs)))
}

pub(in crate::commands::local_api) async fn sync_job_detail(
    state: &AppState,
    task_id: &str,
) -> AppResult<Value> {
    if let Some(job) = state.sync_jobs.get(task_id).await? {
        return Ok(code_ok(job.to_value()?));
    }
    Ok(json!({
        "code": 404,
        "message": "同步任务不存在",
        "data": null
    }))
}

pub(in crate::commands::local_api) async fn cancel_sync_job(
    state: &AppState,
    task_id: &str,
) -> AppResult<Value> {
    if let Some(job) = state
        .sync_jobs
        .cancel_job(task_id, "同步任务已取消")
        .await?
    {
        return Ok(code_ok(job.to_value()?));
    }
    Ok(json!({
        "code": 404,
        "message": "同步任务不存在",
        "data": null
    }))
}
