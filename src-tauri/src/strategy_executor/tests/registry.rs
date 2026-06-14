use serde_json::{json, Map};
use std::fs;

use super::helpers::{
    prepare_python_runner, runtime_strategy_source, temp_project_root, write_runtime_strategy,
    TEST_REGISTRY_LOCK,
};
use crate::strategy_executor::{
    merge_default_params, register, scan_and_register, types::RuntimeStrategyMeta,
};

#[test]
fn merge_default_params_uses_runtime_defaults_and_context_params() {
    let _guard = TEST_REGISTRY_LOCK.lock().unwrap();
    register(RuntimeStrategyMeta {
        strategy_id: "test_runtime_defaults".to_string(),
        strategy_name: "Test Runtime Defaults".to_string(),
        description: String::new(),
        strategy_type: "single_symbol_strategy".to_string(),
        data_requirements: json!({}),
        runtime_config: json!({
            "params": {
                "fast_window": 120,
                "slow_window": 720,
                "signal_model": "overextension_short",
                "runtime_only": true
            }
        }),
        visualization: json!({}),
        decision_contract: json!({}),
        file_name: "test_runtime_defaults.py".to_string(),
    });

    let mut context_params = Map::new();
    context_params.insert("fast_window".to_string(), json!(96));
    context_params.insert("leverage".to_string(), json!(5));

    let merged = merge_default_params("test_runtime_defaults", context_params);
    assert_eq!(merged.get("fast_window"), Some(&json!(96)));
    assert_eq!(merged.get("slow_window"), Some(&json!(720)));
    assert_eq!(
        merged.get("signal_model"),
        Some(&json!("overextension_short"))
    );
    assert_eq!(merged.get("leverage"), Some(&json!(5)));
    assert_eq!(merged.get("runtime_only"), Some(&json!(true)));
}

#[test]
fn scan_registers_runtime_strategies_for_live_runtime_lookup() {
    let _guard = TEST_REGISTRY_LOCK.lock().unwrap();
    let root = temp_project_root("scan_registers_runtime_strategy_for_live_runtime_lookup");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        "runtime/test_scan_strategy.py",
        &runtime_strategy_source(
            "test_scan_strategy",
            "Test Scan Strategy",
            r#"
def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}}
"#,
        ),
    );
    let metas = scan_and_register(&root).unwrap();

    assert!(metas
        .iter()
        .any(|meta| meta.file_name == "runtime/test_scan_strategy.py"));
    let meta = crate::strategy_executor::get_meta("test_scan_strategy")
        .expect("scan should register runtime metadata");
    assert_eq!(meta.strategy_name, "Test Scan Strategy");
    fs::remove_dir_all(root).ok();
}

#[test]
fn scan_and_register_reuses_metadata_when_strategy_files_are_unchanged() {
    let _guard = TEST_REGISTRY_LOCK.lock().unwrap();
    let root = temp_project_root("scan_and_register_reuses_metadata");
    let strategies = root.join("strategies").join("runtime");
    prepare_python_runner(&root);

    let strategy_path = strategies.join("cached_strategy.py");
    let source = runtime_strategy_source(
        "cached_scan_strategy",
        "Cached Scan Strategy",
        r#"
from pathlib import Path

_counter = Path(__file__).with_suffix(".count")
_count = int(_counter.read_text() or "0") if _counter.exists() else 0
_counter.write_text(str(_count + 1))

def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}}
"#,
    );
    write_runtime_strategy(&root, "runtime/cached_strategy.py", &source);

    let first = scan_and_register(&root).unwrap();
    let second = scan_and_register(&root).unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(
        first[0].strategy_id, "cached_scan_strategy",
        "sanity check that the strategy was discovered"
    );
    assert_eq!(
        fs::read_to_string(strategies.join("cached_strategy.count")).unwrap(),
        "1",
        "unchanged files should not start Python discovery again"
    );

    fs::write(&strategy_path, format!("{source}\n# force length change\n")).unwrap();
    scan_and_register(&root).unwrap();
    assert_eq!(
        fs::read_to_string(strategies.join("cached_strategy.count")).unwrap(),
        "2",
        "changed files must invalidate the metadata cache"
    );

    fs::remove_dir_all(root).ok();
}
