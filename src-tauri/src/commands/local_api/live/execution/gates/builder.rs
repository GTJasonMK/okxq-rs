use serde_json::{json, Value};

pub(in crate::commands::local_api::live::execution) fn execution_gate(
    key: &str,
    label: &str,
    status: &str,
    passed: bool,
    blocking: bool,
    detail: &str,
) -> Value {
    json!({
        "key": key,
        "label": label,
        "status": status,
        "passed": passed,
        "blocking": blocking,
        "detail": detail,
    })
}
