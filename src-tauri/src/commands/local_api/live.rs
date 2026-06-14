mod candles;
mod control;
mod diagnostics;
mod execution;
mod numbers;

use serde_json::Value;

use crate::{
    commands::local_api::LocalApiRequest,
    error::{AppError, AppResult},
};

pub(super) use self::control::{
    live_execution_logs, live_execution_plans, live_strategy_equity, live_strategy_orders,
    live_strategy_status, start_live_strategy, stop_live_strategy,
};
pub(super) use self::diagnostics::live_decision_diagnostics;
pub(in crate::commands::local_api::live) use self::numbers::round6;

pub(in crate::commands::local_api::live) fn request_live_strategy_mode(
    req: &LocalApiRequest,
    fallback: &str,
) -> AppResult<String> {
    match req.body.get("mode").or_else(|| req.params.get("mode")) {
        Some(Value::String(mode)) if !mode.trim().is_empty() => normalize_live_strategy_mode(mode),
        Some(Value::String(_)) | None => normalize_live_strategy_mode(fallback),
        Some(_) => Err(AppError::Validation(
            "实时策略运行模式 mode 必须是字符串 live 或 simulated".to_string(),
        )),
    }
}

pub(in crate::commands::local_api::live) fn normalize_live_strategy_mode(
    mode: &str,
) -> AppResult<String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "live" => Ok("live".to_string()),
        "simulated" => Ok("simulated".to_string()),
        other => Err(AppError::Validation(format!(
            "实时策略运行模式只支持 live 或 simulated，收到 {other}"
        ))),
    }
}
