use std::{collections::BTreeSet, time::Instant};

use serde_json::{json, Value};

use crate::{strategy_engine::StrategyConfig, strategy_executor::RuntimeCandleRequirement};

use super::super::super::runtime_helpers::canonical_timeframe;

pub(super) fn emit_context_log(
    on_context_event: &mut (dyn FnMut(&Value) + Send),
    stage: &str,
    level: &str,
    message: impl Into<String>,
    details: Value,
) {
    let event = json!({
        "event": "strategy_log",
        "stage": stage,
        "level": level,
        "message": message.into(),
        "details": {
            "source": "rust_context",
            "data": details,
        },
    });
    on_context_event(&event);
}

pub(super) fn should_log_context_milestone(index: usize, total: usize) -> bool {
    total <= 5 || index == 0 || index + 1 == total || (index + 1) % 5 == 0
}

pub(super) fn candle_requirement_shape_counts(
    requirements: &[RuntimeCandleRequirement],
) -> (usize, usize) {
    let mut symbols = BTreeSet::new();
    let mut timeframes = BTreeSet::new();
    for requirement in requirements {
        let symbol = requirement.symbol.trim().to_uppercase();
        if !symbol.is_empty() {
            symbols.insert(symbol);
        }
        let timeframe = canonical_timeframe(&requirement.timeframe)
            .unwrap_or_else(|| requirement.timeframe.trim())
            .to_string();
        if !timeframe.is_empty() {
            timeframes.insert(timeframe);
        }
    }
    (symbols.len(), timeframes.len())
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn is_primary_requirement(
    requirement: &RuntimeCandleRequirement,
    config: &StrategyConfig,
) -> bool {
    requirement.symbol == config.symbol
        && requirement
            .inst_type
            .eq_ignore_ascii_case(&config.inst_type)
        && requirement.timeframe == config.timeframe
}
