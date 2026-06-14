use serde_json::Value;

use super::builder::execution_gate;

pub(in crate::commands::local_api::live::execution) fn execution_mode_gate(
    runtime_active: bool,
    target_matches_runtime: bool,
    execution_mode: &str,
    intent: &str,
) -> Value {
    if intent == "hold" {
        return execution_gate(
            "execution_mode",
            "执行模式",
            "skip",
            true,
            false,
            "无可执行动作不会记录订单",
        );
    }
    if !runtime_active {
        return execution_gate(
            "execution_mode",
            "执行模式",
            "wait",
            false,
            false,
            "策略未运行",
        );
    }
    if !target_matches_runtime {
        return execution_gate(
            "execution_mode",
            "执行模式",
            "block",
            false,
            true,
            "诊断目标不是当前运行目标",
        );
    }
    let detail = match execution_mode {
        "exchange_live" => "策略运行会提交 OKX 实盘订单",
        "exchange_demo" => "策略运行会提交 OKX 模拟盘订单",
        _ => "实时策略执行模式未知，启动时会被后端拒绝",
    };
    execution_gate("execution_mode", "执行模式", "pass", true, false, detail)
}
