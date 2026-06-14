use serde_json::{json, Value};

pub(super) fn add_issue(integrity: &mut Value, issue: &str) {
    let Some(object) = integrity.as_object_mut() else {
        return;
    };
    let issues = object.entry("issues").or_insert_with(|| json!([]));
    let Some(items) = issues.as_array_mut() else {
        *issues = json!([issue]);
        return;
    };
    if !items.iter().any(|item| item.as_str() == Some(issue)) {
        items.push(json!(issue));
    }
}

pub(super) fn promote_status(integrity: &mut Value, candidate: &str) {
    let Some(object) = integrity.as_object_mut() else {
        return;
    };
    let current = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status_rank(candidate) > status_rank(current) {
        object.insert("status".to_string(), json!(candidate));
        object.insert("label".to_string(), json!(integrity_label(candidate)));
    }
}

pub(super) fn action_contract_mismatch(runtime_contract: &str, history_contract: &str) -> bool {
    let runtime_contract = runtime_contract.trim();
    let history_contract = history_contract.trim();
    if runtime_contract.is_empty() || history_contract.is_empty() {
        return false;
    }
    if runtime_contract == "no_open_actions" && history_contract == "no_open_actions" {
        return false;
    }
    runtime_contract != history_contract
}

pub(super) fn integrity_label(status: &str) -> &'static str {
    match status {
        "ok" => "结果检查通过",
        "warning" => "结果需要复核",
        "invalid" => "结果不可信",
        "unverified" => "旧结果待复核",
        _ => "结果状态未知",
    }
}

fn status_rank(status: &str) -> i32 {
    match status {
        "invalid" => 4,
        "warning" => 3,
        "unverified" => 2,
        "ok" => 1,
        _ => 0,
    }
}
