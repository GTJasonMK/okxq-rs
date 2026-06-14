use serde_json::{json, Value};

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{code_ok, now_unix_seconds, param_i64, LocalApiRequest},
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
        ("GET", ["health"]) => Ok(json!({"status": "healthy"})),
        ("GET", ["status"]) => crate::commands::system::system_status_value(state).await,
        ("GET", ["api", "system", "okx-outbound-timeline"]) => {
            let window = param_i64(req, "window_seconds", 60).max(1);
            let now = now_unix_seconds();
            let mut snapshot = state.okx_outbound_timeline.snapshot(window, now, 1000);
            if let Some(object) = snapshot.as_object_mut() {
                object.insert("governor".to_string(), state.token_bucket.snapshot().await);
            }
            Ok(code_ok(snapshot))
        }
        _ => unsupported_route(method, path),
    }
}
