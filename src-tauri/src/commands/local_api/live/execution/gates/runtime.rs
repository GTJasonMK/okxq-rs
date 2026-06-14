use serde_json::Value;

use crate::live_strategy::LiveStrategyStatus;

use super::builder::execution_gate;

pub(in crate::commands::local_api::live::execution) fn runtime_gate(
    runtime_active: bool,
    target_matches_runtime: bool,
    status: &LiveStrategyStatus,
) -> Value {
    if !runtime_active {
        return execution_gate(
            "runtime",
            "运行状态",
            "wait",
            false,
            false,
            "策略未运行，当前结果用于配置预览",
        );
    }
    if !target_matches_runtime {
        return execution_gate(
            "runtime",
            "运行状态",
            "block",
            false,
            true,
            &format!(
                "当前运行 {} · {} · {}，与诊断目标不一致",
                status.strategy_id, status.symbol, status.timeframe
            ),
        );
    }
    execution_gate(
        "runtime",
        "运行状态",
        "pass",
        true,
        false,
        "当前运行目标一致",
    )
}
