use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{code_ok, scanner, LocalApiRequest},
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
        ("POST", ["api", "scanner", "profiles"]) => {
            scanner::create_scanner_profile(state, req).await
        }
        ("GET", ["api", "scanner", "profiles"]) => scanner::scanner_profiles(state).await,
        ("PUT", ["api", "scanner", "profiles", profile_id]) => {
            scanner::update_scanner_profile(state, profile_id, req).await
        }
        ("DELETE", ["api", "scanner", "profiles", profile_id]) => {
            scanner::delete_scanner_profile(state, profile_id).await
        }
        ("POST", ["api", "scanner", "scan"]) => scanner::run_scan(state, req).await,
        ("POST", ["api", "scanner", "scan", profile_id]) => {
            scanner::run_profile_scan(state, profile_id).await
        }
        ("GET", ["api", "scanner", "results"]) => scanner::scanner_results(state, req).await,
        ("GET", ["api", "scanner", "conditions"]) => Ok(code_ok(scanner::scanner_conditions())),
        _ => unsupported_route(method, path),
    }
}
