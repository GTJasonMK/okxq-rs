use serde_json::{Map, Value};
use tauri::State;

use crate::{app_state::AppState, config::AddWatchedSymbolRequest, error::AppResult};

#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(Value::Object(state.preferences.load().await?))
}

#[tauri::command]
pub async fn get_preference(state: State<'_, AppState>, key: String) -> AppResult<Value> {
    Ok(state.preferences.get(&key).await?.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn save_preferences(
    state: State<'_, AppState>,
    payload: Map<String, Value>,
) -> AppResult<Value> {
    state.preferences.save_all(payload.clone()).await?;
    Ok(Value::Object(payload))
}

#[tauri::command]
pub async fn update_preferences(
    state: State<'_, AppState>,
    payload: Map<String, Value>,
) -> AppResult<Value> {
    Ok(Value::Object(state.preferences.merge(payload).await?))
}

#[tauri::command]
pub async fn delete_preference(state: State<'_, AppState>, key: String) -> AppResult<Value> {
    Ok(Value::Object(state.preferences.delete(&key).await?))
}

#[tauri::command]
pub async fn get_watched_symbols(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": state.preferences.watched_symbols().await?
    }))
}

#[tauri::command]
pub async fn add_watched_symbol(
    state: State<'_, AppState>,
    payload: AddWatchedSymbolRequest,
) -> AppResult<Value> {
    let result = state.preferences.add_watched_symbol(payload).await?;
    Ok(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": result
    }))
}
