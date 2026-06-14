use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use super::status::{action_contract_mismatch, integrity_label};

pub(in crate::commands::local_api::backtest) fn result_integrity_from_summary_columns(
    row: &SqliteRow,
) -> Option<Value> {
    let summary_type = optional_string_column(row, "runtime_action_summary_type");
    let action_count = optional_i64_column(row, "strategy_action_count");
    let warning_count = optional_i64_column(row, "runtime_summary_warning_count");
    let planned_exit_contract = optional_string_column(row, "planned_exit_contract");
    let history_contract_status = optional_string_column(row, "history_action_contract_status");
    let engine_version = optional_string_column(row, "engine_version");
    let history_open_action_count = optional_i64_column(row, "history_open_action_count");
    let history_open_actions_with_planned_exit =
        optional_i64_column(row, "history_open_actions_with_planned_exit");
    let has_columns = summary_type.is_some()
        || action_count.is_some()
        || warning_count.is_some()
        || planned_exit_contract.is_some()
        || history_contract_status.is_some()
        || engine_version.is_some()
        || history_open_action_count.is_some()
        || history_open_actions_with_planned_exit.is_some();
    if !has_columns {
        return None;
    }

    let summary_present = summary_type.flatten().as_deref() == Some("object");
    let action_count = action_count.flatten().unwrap_or(0).max(0);
    let warning_count = warning_count.flatten().unwrap_or(0).max(0);
    let planned_exit_contract = planned_exit_contract.flatten().unwrap_or_default();
    let history_contract_status = history_contract_status.flatten().unwrap_or_default();
    let engine_version = engine_version.flatten().unwrap_or_default();
    let history_open_action_count = history_open_action_count.flatten().unwrap_or(0).max(0);
    let history_open_actions_with_planned_exit = history_open_actions_with_planned_exit
        .flatten()
        .unwrap_or(0)
        .max(0);
    let summary_evidence =
        summary_present || warning_count > 0 || !planned_exit_contract.trim().is_empty();
    if !summary_evidence && action_count == 0 && history_contract_status.is_empty() {
        return None;
    }

    let mut issues = Vec::<String>::new();
    if !summary_present
        && (action_count > 0
            || warning_count > 0
            || !planned_exit_contract.is_empty()
            || !history_contract_status.is_empty())
    {
        issues.push("runtime_action_summary_missing".to_string());
    }
    if warning_count > 0 {
        issues.push("runtime_action_summary_warnings".to_string());
    }
    if planned_exit_contract == "planned_exit_missing" {
        issues.push("planned_exit_missing".to_string());
    } else if planned_exit_contract == "planned_exit_partial" {
        issues.push("planned_exit_partial".to_string());
    }
    if history_contract_status == "planned_exit_missing" {
        issues.push("strategy_history_planned_exit_missing".to_string());
    } else if history_contract_status == "planned_exit_partial" {
        issues.push("strategy_history_planned_exit_partial".to_string());
    }
    let contract_mismatch = action_contract_mismatch(
        planned_exit_contract.as_str(),
        history_contract_status.as_str(),
    );
    if contract_mismatch {
        issues.push("strategy_runtime_action_contract_mismatch".to_string());
    }

    let status = if planned_exit_contract == "planned_exit_missing"
        || history_contract_status == "planned_exit_missing"
        || contract_mismatch
    {
        "invalid"
    } else if warning_count > 0
        || planned_exit_contract == "planned_exit_partial"
        || history_contract_status == "planned_exit_partial"
    {
        "warning"
    } else if !summary_present && (action_count > 0 || !history_contract_status.is_empty()) {
        "unverified"
    } else {
        "ok"
    };

    issues.sort();
    issues.dedup();
    Some(json!({
        "status": status,
        "label": integrity_label(status),
        "source": "history_summary",
        "issues": issues,
        "runtime_action_summary_present": summary_present,
        "strategy_action_count": action_count,
        "runtime_summary_warning_count": warning_count,
        "planned_exit_contract": planned_exit_contract,
        "history_action_contract_status": history_contract_status,
        "history_open_action_count": history_open_action_count,
        "history_open_actions_with_planned_exit": history_open_actions_with_planned_exit,
        "engine_version": engine_version,
        "strategy_protocol": "evaluate",
    }))
}

fn optional_string_column(row: &SqliteRow, name: &str) -> Option<Option<String>> {
    row.try_get::<Option<String>, _>(name).ok()
}

fn optional_i64_column(row: &SqliteRow, name: &str) -> Option<Option<i64>> {
    row.try_get::<Option<i64>, _>(name).ok()
}
