use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{body_string, LocalApiRequest},
    error::AppResult,
};

/// POST /api/agent/analyze/python — 在安全沙箱中执行 Python 代码
pub(crate) async fn analyze_python(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let code = body_string(req, "code", "");
    if code.trim().is_empty() {
        return Ok(json!({"ok": false, "error": "代码不能为空"}));
    }

    let runner_path = state.paths.root.join("src-tauri/python/code_runner.py");

    let command = json!({
        "action": "execute",
        "code": code,
    });

    match crate::strategy_executor::call_python_runner(&runner_path.to_string_lossy(), command) {
        Ok(result) => Ok(json!({
            "ok": true,
            "data": result,
        })),
        Err(error) => Ok(json!({
            "ok": false,
            "error": error.to_string(),
        })),
    }
}
