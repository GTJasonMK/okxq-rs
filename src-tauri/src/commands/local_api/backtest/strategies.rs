use serde_json::{json, Value};

use crate::{app_state::AppState, error::AppResult, strategy_executor};

pub(in crate::commands::local_api) async fn available_strategies(
    state: &AppState,
) -> AppResult<Value> {
    let mut items = Vec::new();
    for meta in strategy_executor::scan_and_register(&state.paths.root)? {
        items.push(json!({
            "id": meta.strategy_id,
            "name": meta.strategy_name,
            "description": meta.description,
            "strategy_type": meta.strategy_type,
            "data_requirements": meta.data_requirements,
            "runtime": meta.runtime_config,
            "visualization": meta.visualization,
            "decision_contract": meta.decision_contract,
            "file_name": meta.file_name
        }));
    }
    Ok(Value::Array(items))
}
