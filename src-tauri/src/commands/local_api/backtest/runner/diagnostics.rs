use serde_json::{json, Value};

use crate::strategy_engine::StrategyConfig;

pub(in crate::commands::local_api::backtest) fn compact_backtest_diagnostics_enabled(
    config: &StrategyConfig,
) -> bool {
    json_bool_param(
        &config.params,
        &[
            "compact_backtest_diagnostics",
            "backtest_compact_diagnostics",
            "compact_diagnostics",
        ],
        false,
    )
}

pub(in crate::commands::local_api::backtest) fn compact_strategy_diagnostics(
    diagnostics: &Value,
) -> Value {
    let mut next = diagnostics.clone();
    compact_strategy_diagnostics_in_place(&mut next);
    next
}

fn json_bool_param(params: &Value, keys: &[&str], default: bool) -> bool {
    for key in keys {
        let Some(value) = params.get(*key) else {
            continue;
        };
        return match value {
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_f64().is_some_and(|item| item != 0.0),
            Value::String(value) => {
                let normalized = value.trim().to_ascii_lowercase();
                !matches!(
                    normalized.as_str(),
                    "" | "0" | "false" | "no" | "off" | "none" | "null"
                )
            }
            _ => default,
        };
    }
    default
}

fn compact_strategy_diagnostics_in_place(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for key in [
        "scoring",
        "score_report",
        "score_report_by_timestamp",
        "ranked_candidates",
        "ranked_by_timestamp",
        "candidate_rows",
        "raw_candidate_rows",
        "debug_rows",
        "raw_rows",
        "trace",
        "model_debug",
    ] {
        if let Some(removed) = object.remove(key) {
            object.insert(format!("{key}_omitted"), compact_omitted_value(&removed));
        }
    }
}

fn compact_omitted_value(value: &Value) -> Value {
    let (value_type, item_count) = match value {
        Value::Array(items) => ("array", items.len()),
        Value::Object(items) => ("object", items.len()),
        Value::String(_) => ("string", 1),
        Value::Number(_) => ("number", 1),
        Value::Bool(_) => ("bool", 1),
        Value::Null => ("null", 0),
    };
    json!({
        "compacted": true,
        "value_type": value_type,
        "item_count": item_count,
    })
}
