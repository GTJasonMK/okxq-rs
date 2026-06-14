use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::status::{action_contract_mismatch, add_issue, integrity_label, promote_status};

pub(in crate::commands::local_api::backtest) fn result_integrity_from_detail(
    detail: &Value,
) -> Option<Value> {
    let engine_version = detail
        .get("engine_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let history_contract = history_action_contract(detail);
    if let Some(summary) = detail
        .get("runtime_action_summary")
        .filter(|value| value.is_object())
    {
        let mut integrity =
            result_integrity_from_runtime_summary(summary, "runtime_action_summary");
        if let Some(contract) = history_contract {
            apply_history_action_contract(&mut integrity, contract);
        }
        apply_engine_metadata(&mut integrity, detail, engine_version);
        return Some(integrity);
    }

    if let Some(contract) = history_contract {
        let mut integrity = result_integrity_from_history_action_contract(contract);
        apply_engine_metadata(&mut integrity, detail, engine_version);
        return Some(integrity);
    }

    let actions = detail.get("strategy_actions").and_then(Value::as_array)?;
    if actions.is_empty() {
        return None;
    }

    let mut runtime_action_count = 0_i64;
    let mut open_action_count = 0_i64;
    let mut open_actions_with_planned_exit = 0_i64;
    for action in actions {
        let name = action_name(action);
        if is_runtime_action(&name) {
            runtime_action_count += 1;
        }
        if is_runtime_open_action(&name) {
            open_action_count += 1;
            if action_has_planned_exit(action) {
                open_actions_with_planned_exit += 1;
            }
        }
    }
    if runtime_action_count == 0 {
        return None;
    }

    let close_reason_counts = close_reason_counts(detail.get("trades"));
    let close_trade_count = close_reason_counts.values().sum::<i64>();
    let only_risk_or_backtest_end_closes = close_trade_count > 0
        && close_reason_counts.keys().all(|reason| {
            matches!(
                reason.as_str(),
                "stop_loss" | "take_profit" | "end_of_backtest" | "end_of_backtest_missing_mark"
            )
        });

    let mut issues = vec!["runtime_action_summary_missing".to_string()];
    if open_action_count > 0 && open_actions_with_planned_exit == 0 {
        issues.push("runtime_open_actions_missing_planned_exit".to_string());
    }
    if open_action_count > 0
        && open_actions_with_planned_exit == 0
        && only_risk_or_backtest_end_closes
    {
        issues.push("runtime_closes_without_planned_lifecycle".to_string());
    }

    Some(json!({
        "status": "unverified",
        "label": integrity_label("unverified"),
        "source": "legacy_detail",
        "issues": issues,
        "runtime_action_summary_present": false,
        "strategy_action_count": runtime_action_count,
        "open_action_count": open_action_count,
        "open_actions_with_planned_exit": open_actions_with_planned_exit,
        "close_trade_count": close_trade_count,
        "close_reason_counts": close_reason_counts,
    }))
}

fn apply_engine_metadata(integrity: &mut Value, detail: &Value, engine_version: &str) {
    if engine_version.is_empty() {
        return;
    }
    if let Some(object) = integrity.as_object_mut() {
        object.insert("engine_version".to_string(), json!(engine_version));
        object.insert(
            "strategy_protocol".to_string(),
            json!(detail
                .get("strategy_protocol")
                .and_then(Value::as_str)
                .unwrap_or("evaluate")),
        );
    }
}

fn result_integrity_from_runtime_summary(summary: &Value, source: &str) -> Value {
    let planned_exit_contract = summary
        .get("planned_exit_contract")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let warnings = summary
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut issues = warnings.clone();
    if planned_exit_contract == "planned_exit_missing" {
        issues.push("planned_exit_missing".to_string());
    } else if planned_exit_contract == "planned_exit_partial" {
        issues.push("planned_exit_partial".to_string());
    }
    issues.sort();
    issues.dedup();

    let status = if planned_exit_contract == "planned_exit_missing"
        || warnings
            .iter()
            .any(|item| item == "all_closes_deferred_to_backtest_end_without_planned_exits")
    {
        "invalid"
    } else if planned_exit_contract == "planned_exit_partial" || !warnings.is_empty() {
        "warning"
    } else {
        "ok"
    };

    json!({
        "status": status,
        "label": integrity_label(status),
        "source": source,
        "issues": issues,
        "runtime_action_summary_present": true,
        "planned_exit_contract": planned_exit_contract,
        "runtime_summary_warning_count": warnings.len(),
    })
}

fn history_action_contract(detail: &Value) -> Option<&Value> {
    detail
        .get("strategy_diagnostics")
        .and_then(|value| value.get("history_action_contract"))
        .filter(|value| value.is_object())
}

fn result_integrity_from_history_action_contract(contract: &Value) -> Value {
    let status = history_action_contract_status(contract);
    let has_open_actions = contract
        .get("open_action_count")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        > 0;
    let mut issues = Vec::<String>::new();
    if has_open_actions {
        issues.push("runtime_action_summary_missing".to_string());
    }
    if status == "planned_exit_missing" {
        issues.push("strategy_history_planned_exit_missing".to_string());
    } else if status == "planned_exit_partial" {
        issues.push("strategy_history_planned_exit_partial".to_string());
    }
    let integrity_status = if status == "planned_exit_missing" {
        "invalid"
    } else if status == "planned_exit_partial" {
        "warning"
    } else if has_open_actions {
        "unverified"
    } else {
        "ok"
    };
    let mut integrity = json!({
        "status": integrity_status,
        "label": integrity_label(integrity_status),
        "source": "strategy_history_action_contract",
        "issues": issues,
        "runtime_action_summary_present": false,
    });
    apply_history_action_contract(&mut integrity, contract);
    integrity
}

fn apply_history_action_contract(integrity: &mut Value, contract: &Value) {
    let status = history_action_contract_status(contract);
    let open_action_count = contract
        .get("open_action_count")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let open_actions_with_planned_exit = contract
        .get("open_actions_with_planned_exit")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let runtime_contract = integrity
        .get("planned_exit_contract")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(object) = integrity.as_object_mut() {
        object.insert(
            "history_action_contract_status".to_string(),
            json!(status.clone()),
        );
        object.insert(
            "history_open_action_count".to_string(),
            json!(open_action_count),
        );
        object.insert(
            "history_open_actions_with_planned_exit".to_string(),
            json!(open_actions_with_planned_exit),
        );
    }
    if status == "planned_exit_missing" {
        add_issue(integrity, "strategy_history_planned_exit_missing");
        promote_status(integrity, "invalid");
    } else if status == "planned_exit_partial" {
        add_issue(integrity, "strategy_history_planned_exit_partial");
        promote_status(integrity, "warning");
    }
    if action_contract_mismatch(runtime_contract.as_str(), status.as_str()) {
        add_issue(integrity, "strategy_runtime_action_contract_mismatch");
        promote_status(integrity, "invalid");
    }
}

fn history_action_contract_status(contract: &Value) -> String {
    contract
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn close_reason_counts(trades: Option<&Value>) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::<String, i64>::new();
    let Some(trades) = trades.and_then(Value::as_array) else {
        return counts;
    };
    for trade in trades {
        if trade_action(trade) != "close" {
            continue;
        }
        let reason = trade
            .get("reason")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("unknown")
            .to_string();
        *counts.entry(reason).or_default() += 1;
    }
    counts
}

fn trade_action(trade: &Value) -> String {
    trade
        .get("metadata")
        .and_then(|value| value.get("action"))
        .and_then(Value::as_str)
        .or_else(|| trade.get("action").and_then(Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn action_name(action: &Value) -> String {
    action
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn is_runtime_action(name: &str) -> bool {
    matches!(
        name,
        "open_position"
            | "close_position"
            | "place_risk_order"
            | "cancel_order"
            | "modify_order"
            | "hold"
    )
}

fn is_runtime_open_action(name: &str) -> bool {
    matches!(name, "open_position")
}

fn action_has_planned_exit(action: &Value) -> bool {
    ["exit_time", "planned_exit_time"]
        .iter()
        .any(|key| action.get(*key).is_some_and(|value| !value.is_null()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn detail_integrity_flags_legacy_runtime_actions_without_planned_exit() {
        let detail = json!({
            "strategy_actions": [
                {"action": "open_position", "symbol": "ETH-USDT-SWAP"},
                {"action": "place_risk_order", "symbol": "ETH-USDT-SWAP"}
            ],
            "trades": [
                {"reason": "stop_loss", "metadata": {"action": "close"}},
                {"reason": "end_of_backtest", "metadata": {"action": "close"}}
            ]
        });

        let integrity = result_integrity_from_detail(&detail).expect("legacy runtime actions");

        assert_eq!(integrity["status"].as_str(), Some("unverified"));
        assert_eq!(
            integrity["runtime_action_summary_present"].as_bool(),
            Some(false)
        );
        assert_eq!(integrity["open_action_count"].as_i64(), Some(1));
        assert_eq!(
            integrity["open_actions_with_planned_exit"].as_i64(),
            Some(0)
        );
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("runtime_open_actions_missing_planned_exit")));
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("runtime_closes_without_planned_lifecycle")));
    }

    #[test]
    fn detail_integrity_marks_runtime_summary_complete_as_ok() {
        let detail = json!({
            "runtime_action_summary": {
                "planned_exit_contract": "planned_exit_complete",
                "warnings": []
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("summary integrity");

        assert_eq!(integrity["status"].as_str(), Some("ok"));
        assert_eq!(integrity["issues"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn detail_integrity_marks_missing_planned_exit_summary_as_invalid() {
        let detail = json!({
            "runtime_action_summary": {
                "planned_exit_contract": "planned_exit_missing",
                "warnings": ["all_closes_deferred_to_backtest_end_without_planned_exits"]
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("summary integrity");

        assert_eq!(integrity["status"].as_str(), Some("invalid"));
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("planned_exit_missing")));
    }

    #[test]
    fn historical_live_detail_uses_runtime_summary_before_engine_ok_fallback() {
        let detail = json!({
            "engine_version": "historical_live_v1",
            "strategy_protocol": "evaluate",
            "runtime_action_summary": {
                "planned_exit_contract": "planned_exit_missing",
                "warnings": []
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("summary integrity");

        assert_eq!(integrity["status"].as_str(), Some("invalid"));
        assert_eq!(
            integrity["engine_version"].as_str(),
            Some("historical_live_v1")
        );
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("planned_exit_missing")));
    }

    #[test]
    fn historical_live_detail_with_no_diagnostics_does_not_fabricate_ok_integrity() {
        let detail = json!({
            "engine_version": "historical_live_v1",
            "strategy_protocol": "evaluate"
        });

        assert!(result_integrity_from_detail(&detail).is_none());
    }

    #[test]
    fn detail_integrity_marks_strategy_history_contract_missing_as_invalid() {
        let detail = json!({
            "strategy_diagnostics": {
                "history_action_contract": {
                    "status": "planned_exit_missing",
                    "open_action_count": 3,
                    "open_actions_with_planned_exit": 0
                }
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("strategy contract integrity");

        assert_eq!(integrity["status"].as_str(), Some("invalid"));
        assert_eq!(
            integrity["history_action_contract_status"].as_str(),
            Some("planned_exit_missing")
        );
        assert_eq!(integrity["history_open_action_count"].as_i64(), Some(3));
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("strategy_history_planned_exit_missing")));
    }

    #[test]
    fn detail_integrity_combines_runtime_summary_with_strategy_history_contract() {
        let detail = json!({
            "runtime_action_summary": {
                "planned_exit_contract": "planned_exit_complete",
                "warnings": []
            },
            "strategy_diagnostics": {
                "history_action_contract": {
                    "status": "planned_exit_missing",
                    "open_action_count": 2,
                    "open_actions_with_planned_exit": 0
                }
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("combined integrity");

        assert_eq!(integrity["status"].as_str(), Some("invalid"));
        assert_eq!(
            integrity["history_action_contract_status"].as_str(),
            Some("planned_exit_missing")
        );
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("strategy_runtime_action_contract_mismatch")));
    }

    #[test]
    fn detail_integrity_flags_runtime_summary_missing_but_strategy_contract_complete() {
        let detail = json!({
            "runtime_action_summary": {
                "planned_exit_contract": "planned_exit_missing",
                "warnings": ["open_actions_missing_planned_exit"]
            },
            "strategy_diagnostics": {
                "history_action_contract": {
                    "status": "planned_exit_complete",
                    "open_action_count": 1095,
                    "open_actions_with_planned_exit": 1095
                }
            }
        });

        let integrity = result_integrity_from_detail(&detail).expect("mismatch integrity");

        assert_eq!(integrity["status"].as_str(), Some("invalid"));
        assert_eq!(
            integrity["history_action_contract_status"].as_str(),
            Some("planned_exit_complete")
        );
        assert!(integrity["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("strategy_runtime_action_contract_mismatch")));
    }

    #[test]
    fn detail_integrity_does_not_count_hold_bars_without_exit_time_as_planned_exit() {
        let detail = json!({
            "strategy_actions": [
                {
                    "action": "open_position",
                    "symbol": "ETH-USDT-SWAP",
                    "hold_bars": 40,
                    "planned_hold_bars": 40
                }
            ]
        });

        let integrity = result_integrity_from_detail(&detail).expect("legacy runtime actions");

        assert_eq!(integrity["status"].as_str(), Some("unverified"));
        assert_eq!(integrity["open_action_count"].as_i64(), Some(1));
        assert_eq!(
            integrity["open_actions_with_planned_exit"].as_i64(),
            Some(0)
        );
    }

    #[test]
    fn detail_integrity_counts_all_actions_v1_runtime_actions() {
        let detail = json!({
            "strategy_actions": [
                {"action": "cancel_order", "symbol": "ETH-USDT-SWAP"},
                {"action": "modify_order", "symbol": "ETH-USDT-SWAP"},
                {"action": "hold", "symbol": "ETH-USDT-SWAP"}
            ],
            "trades": []
        });

        let integrity = result_integrity_from_detail(&detail).expect("actions_v1 runtime actions");

        assert_eq!(integrity["status"].as_str(), Some("unverified"));
        assert_eq!(integrity["strategy_action_count"].as_i64(), Some(3));
        assert_eq!(integrity["open_action_count"].as_i64(), Some(0));
        let issues = integrity["issues"].as_array().unwrap();
        assert!(issues
            .iter()
            .any(|item| item.as_str() == Some("runtime_action_summary_missing")));
        assert!(!issues
            .iter()
            .any(|item| item.as_str() == Some("runtime_open_actions_missing_planned_exit")));
    }
}
