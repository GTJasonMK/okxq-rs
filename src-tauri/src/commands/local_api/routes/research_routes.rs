use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{research, LocalApiRequest},
    unsupported_route,
};

pub(super) async fn dispatch(
    state: &AppState,
    req: &LocalApiRequest,
    method: &str,
    path: &str,
    segment_refs: &[&str],
) -> AppResult<Value> {
    match (method, segment_refs) {
        ("GET", ["api", "data-center", "collections", "sessions"]) => {
            research::collection_sessions(state, req).await
        }
        ("GET", ["api", "data-center", "collections", "sessions", session_id]) => {
            research::collection_session_detail(state, session_id).await
        }
        ("POST", ["api", "data-center", "collections", "sessions"]) => {
            research::create_collection_session(state, req).await
        }
        ("POST", ["api", "data-center", "collections", "sessions", session_id, "stop"]) => {
            research::stop_collection_session(state, session_id).await
        }
        ("DELETE", ["api", "data-center", "collections", "sessions", session_id]) => {
            research::delete_collection_session(state, session_id).await
        }
        ("GET", ["api", "data-center", "collections", "census", "status"]) => {
            research::collection_census_status(state).await
        }
        ("GET", ["api", "research-platform", "sessions"]) => {
            research::collection_sessions(state, req).await
        }
        ("GET", ["api", "research-platform", "sessions", session_id]) => {
            research::collection_session_detail(state, session_id).await
        }
        ("POST", ["api", "research-platform", "sessions"]) => {
            research::create_collection_session(state, req).await
        }
        ("POST", ["api", "research-platform", "sessions", session_id, "stop"]) => {
            research::stop_collection_session(state, session_id).await
        }
        ("GET", ["api", "research-platform", "census", "status"]) => {
            research::collection_census_status(state).await
        }
        ("GET", ["api", "research-platform", "datasets"]) => {
            research::research_datasets(state, req).await
        }
        ("POST", ["api", "research-platform", "datasets"]) => {
            research::create_research_dataset(state, req).await
        }
        ("GET", ["api", "research-platform", "datasets", dataset_id, "preview"]) => {
            research::research_dataset_preview(state, dataset_id).await
        }
        ("GET", ["api", "research-platform", "datasets", dataset_id]) => {
            research::research_dataset_detail(state, dataset_id).await
        }
        ("DELETE", ["api", "research-platform", "datasets", dataset_id]) => {
            research::delete_research_dataset(state, dataset_id).await
        }
        ("GET", ["api", "research-platform", "training-runs"]) => {
            research::research_training_runs(state, req).await
        }
        ("POST", ["api", "research-platform", "training-runs"]) => {
            research::create_research_training_run(state, req).await
        }
        ("GET", ["api", "research-platform", "training-runs", run_id]) => {
            research::research_training_run_detail(state, run_id).await
        }
        ("GET", ["api", "trend-research", "inference"]) => {
            research::trend_research_inference(state, req).await
        }
        ("GET", ["api", "trend-research", "process"]) => {
            research::trend_research_process(state, req).await
        }
        ("GET", ["api", "trend-research", "diagnostics"]) => {
            research::trend_research_diagnostics(state, req).await
        }
        ("GET", ["api", "trend-research", "feature-bars", inst_id]) => {
            research::trend_research_feature_bars(state, inst_id, req).await
        }
        ("GET", ["api", "trend-research", "factors", inst_id]) => {
            research::trend_research_factors(state, inst_id, req).await
        }
        ("GET", ["api", "trend-research", "factor-series", inst_id]) => {
            research::trend_research_factor_series(state, inst_id, req).await
        }
        ("GET", ["api", "trend-research", "config"]) => {
            research::trend_research_config(state).await
        }
        ("PUT", ["api", "trend-research", "config"]) => {
            research::update_trend_research_config(state, req).await
        }
        ("GET", ["api", "trend-research", "model"]) => research::trend_research_model(state).await,
        ("GET", ["api", "trend-research", "training-run"]) => {
            research::trend_research_training_run(state).await
        }
        ("POST", ["api", "trend-research", "model", "retrain"]) => {
            research::retrain_trend_research_model(state, req).await
        }
        ("POST", ["api", "research", "factors", "compute"]) => {
            research::compute_research_factors(state, req).await
        }
        ("POST", ["api", "research", "dataset", "build"]) => {
            research::build_research_dataset(state, req).await
        }
        ("GET", ["api", "research", "dataset", "splits", dataset_id]) => {
            research::research_dataset_splits(state, dataset_id, req).await
        }
        ("POST", ["api", "research", "model", "train"]) => {
            research::train_research_model(state, req).await
        }
        _ => unsupported_route(method, path),
    }
}
