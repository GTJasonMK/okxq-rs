use super::*;

mod chat;
mod llm;
mod records;
mod tools;

pub(crate) use self::chat::assistant_agent_chat;
use self::records::{
    append_assistant_message, assistant_session_detail_value, create_assistant_session_record,
    fetch_assistant_session,
};
pub(crate) use self::records::{
    assistant_agent_session_detail, assistant_agent_sessions, assistant_level_snapshot,
    assistant_level_snapshots, assistant_order_draft, assistant_order_drafts,
    assistant_patrol_config, assistant_patrol_run, assistant_patrol_runs, assistant_patrol_status,
    assistant_run_patrol_now, assistant_update_patrol_config, confirm_assistant_order_draft,
    create_assistant_agent_session, create_assistant_level_snapshot, create_assistant_order_draft,
};

pub(super) async fn assistant_status(state: &AppState) -> AppResult<Value> {
    let cfg = state.config.read().await;
    Ok(code_ok(json!({
        "enabled": cfg.assistant.enabled,
        "configured": cfg.assistant.is_configured(),
        "provider_name": cfg.assistant.provider_name,
        "model": cfg.assistant.model,
        "runtime": "rust"
    })))
}

pub(super) fn assistant_agent_tools() -> AppResult<Value> {
    Ok(code_ok(json!([
            {"name": "market_context", "description": "读取当前前端传入的行情上下文"},
            {"name": "local_session_memory", "description": "保存并读取本地分析会话"}
    ])))
}
