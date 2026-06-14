use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use tauri::State;

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};

mod agent;
mod assistant;
mod backtest;
mod helpers;
mod inventory;
mod journal;
mod live;
mod market;
mod market_ops;
mod research;
mod routes;
mod scanner;
mod tick_collector;
mod trading;

pub(crate) use self::helpers::*;
pub(crate) use self::inventory::{
    cancel_related_sync_jobs, delete_marked_symbol_related_data, mark_inventory_deletion_requested,
};

#[derive(Debug, Deserialize)]
pub struct LocalApiRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub body: Value,
}

#[tauri::command]
pub async fn local_api_request(
    state: State<'_, AppState>,
    req: LocalApiRequest,
) -> AppResult<Value> {
    let method = req.method.trim().to_uppercase();
    let path = normalize_path(&req.path);
    let segments = path_segments(&path);
    let segment_refs = segments.iter().map(String::as_str).collect::<Vec<_>>();
    let started = Instant::now();

    tracing::debug!(
        method = method.as_str(),
        path = path.as_str(),
        params = req.params.len(),
        has_body = !req.body.is_null(),
        "local API request started"
    );

    let result = routes::dispatch_local_api_request(
        &state,
        &req,
        method.as_str(),
        path.as_str(),
        &segment_refs,
    )
    .await;
    let duration_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(value) => {
            let outcome = response_outcome(value);
            if outcome.success {
                tracing::debug!(
                    method = method.as_str(),
                    path = path.as_str(),
                    duration_ms,
                    code = outcome.code,
                    message = outcome.message.as_deref().unwrap_or(""),
                    "local API request completed"
                );
            } else {
                tracing::warn!(
                    method = method.as_str(),
                    path = path.as_str(),
                    duration_ms,
                    code = outcome.code,
                    message = outcome.message.as_deref().unwrap_or(""),
                    "local API request returned failure payload"
                );
            }
        }
        Err(error) => {
            tracing::error!(
                method = method.as_str(),
                path = path.as_str(),
                duration_ms,
                error = %error,
                "local API request failed"
            );
        }
    }
    result
}
