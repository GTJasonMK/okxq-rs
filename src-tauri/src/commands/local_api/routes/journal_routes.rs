use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{journal, LocalApiRequest},
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
        ("POST", ["api", "journal", "entries"]) => journal::create_journal_entry(state, req).await,
        ("GET", ["api", "journal", "entries"]) => journal::journal_entries(state, req).await,
        ("GET", ["api", "journal", "entries", entry_id]) => {
            journal::journal_entry(state, entry_id).await
        }
        ("PUT", ["api", "journal", "entries", entry_id]) => {
            journal::update_journal_entry(state, entry_id, req).await
        }
        ("DELETE", ["api", "journal", "entries", entry_id]) => {
            journal::delete_journal_entry(state, entry_id).await
        }
        ("GET", ["api", "journal", "tags"]) => journal::journal_tags(state).await,
        ("GET", ["api", "journal", "stats"]) => journal::journal_stats(state, req).await,
        _ => unsupported_route(method, path),
    }
}
