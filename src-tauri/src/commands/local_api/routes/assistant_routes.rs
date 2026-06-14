use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{assistant, LocalApiRequest},
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
        ("GET", ["api", "assistant", "status"]) => assistant::assistant_status(state).await,
        ("GET", ["api", "assistant", "agent", "tools"]) => assistant::assistant_agent_tools(),
        ("GET", ["api", "assistant", "agent", "sessions"]) => {
            assistant::assistant_agent_sessions(state, req).await
        }
        ("POST", ["api", "assistant", "agent", "sessions"]) => {
            assistant::create_assistant_agent_session(state, req).await
        }
        ("GET", ["api", "assistant", "agent", "sessions", session_id]) => {
            assistant::assistant_agent_session_detail(state, session_id).await
        }
        ("POST", ["api", "assistant", "agent", "chat"]) => {
            assistant::assistant_agent_chat(state, req).await
        }
        ("POST", ["api", "assistant", "agent", "order-drafts"]) => {
            assistant::create_assistant_order_draft(state, req).await
        }
        ("GET", ["api", "assistant", "agent", "order-drafts"]) => {
            assistant::assistant_order_drafts(state, req).await
        }
        ("POST", ["api", "assistant", "agent", "order-drafts", draft_id, "confirm"]) => {
            assistant::confirm_assistant_order_draft(state, draft_id).await
        }
        ("GET", ["api", "assistant", "agent", "order-drafts", draft_id]) => {
            assistant::assistant_order_draft(state, draft_id).await
        }
        ("GET", ["api", "assistant", "agent", "level-snapshots"]) => {
            assistant::assistant_level_snapshots(state, req).await
        }
        ("POST", ["api", "assistant", "agent", "level-snapshots"]) => {
            assistant::create_assistant_level_snapshot(state, req).await
        }
        ("GET", ["api", "assistant", "agent", "level-snapshots", snapshot_id]) => {
            assistant::assistant_level_snapshot(state, snapshot_id).await
        }
        ("GET", ["api", "assistant", "agent", "patrol", "status"]) => {
            assistant::assistant_patrol_status()
        }
        ("GET", ["api", "assistant", "agent", "patrol", "config"]) => {
            assistant::assistant_patrol_config()
        }
        ("PUT", ["api", "assistant", "agent", "patrol", "config"]) => {
            assistant::assistant_update_patrol_config(req)
        }
        ("POST", ["api", "assistant", "agent", "patrol", "run-now"]) => {
            assistant::assistant_run_patrol_now(state, req).await
        }
        ("GET", ["api", "assistant", "agent", "patrol", "runs"]) => {
            assistant::assistant_patrol_runs(state, req).await
        }
        ("GET", ["api", "assistant", "agent", "patrol", "runs", run_id]) => {
            assistant::assistant_patrol_run(state, run_id).await
        }
        _ => unsupported_route(method, path),
    }
}
