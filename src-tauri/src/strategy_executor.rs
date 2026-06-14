//! 运行策略执行器 — 通过 Python sidecar 进程加载 .py 策略文件并执行动作决策。
//!
//! 通信协议：stdin 逐行 JSON 命令，stdout 逐行 JSON 响应。

mod context;
mod files;
mod funding_context;
mod registry;
mod runner;
pub(crate) mod types;

pub use context::{
    candle_requirements, candle_tree, context_cache_stamp, funding_requirements,
    orderbook_requirements, state_context_requirements, strategy_context, RuntimeCandleRequirement,
    RuntimeFundingRequirement, RuntimeOrderbookRequirement, RuntimeStateRequirements,
    StrategyContextInput,
};
#[cfg(test)]
pub use files::{discoverable_strategy_files, normalize_strategy_file_name};
pub use files::{discoverable_strategy_ids_fast, runtime_execution_stamp};
pub(crate) use funding_context::{
    funding_context_value_from_history, load_local_funding_history_checked,
    load_local_funding_history_until_checked, local_funding_table_exists,
    normalize_candle_requirement, normalize_funding_requirement, normalize_orderbook_requirement,
    normalize_runtime_inst_id,
};
#[cfg(test)]
pub use registry::register;
pub use registry::{ensure_registered, get_meta, merge_default_params, scan_and_register};
#[cfg(test)]
pub use runner::discover_runtime_strategy;
pub(crate) use runner::PythonRunnerSession;
pub use runner::{
    call_python_runner, compute_runtime_decision_with_context_and_events,
    compute_runtime_diagnostics_decision_with_context, runtime_diagnostics_response_from_decision,
};
#[cfg(test)]
pub use runner::{
    compute_runtime_decision, compute_runtime_decision_with_context,
    compute_runtime_decision_with_context_and_progress, compute_runtime_diagnostics,
};

#[cfg(test)]
mod tests;
