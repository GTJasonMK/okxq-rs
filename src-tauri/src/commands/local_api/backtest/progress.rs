use serde_json::{json, Value};

use crate::{
    app_state::AppState, backtest_progress::idle_progress, commands::local_api::code_ok,
    error::AppResult,
};

pub(in crate::commands::local_api) async fn backtest_progress(
    state: &AppState,
    run_id: &str,
) -> AppResult<Value> {
    let progress = state
        .backtest_progress
        .get(run_id)
        .unwrap_or_else(|| idle_progress(run_id));
    Ok(code_ok(json!(progress)))
}
