use super::super::super::*;

pub(in crate::commands::local_api) async fn watched_symbols(state: &AppState) -> AppResult<Value> {
    Ok(code_ok(serde_json::to_value(
        state.preferences.watched_symbols().await?,
    )?))
}
