use std::fs;

use super::helpers::{
    prepare_python_runner, runtime_strategy_source, temp_project_root, write_runtime_strategy,
};
use crate::strategy_executor::{
    discover_runtime_strategy, discoverable_strategy_files, discoverable_strategy_ids_fast,
    normalize_strategy_file_name, runtime_execution_stamp,
};

#[test]
fn normalize_strategy_file_name_accepts_runtime_relative_paths() {
    assert_eq!(
        normalize_strategy_file_name("runtime/multi_timeframe_dual_v12.py").unwrap(),
        "runtime/multi_timeframe_dual_v12.py"
    );
    assert!(normalize_strategy_file_name("../secret.py").is_err());
    assert!(normalize_strategy_file_name("/tmp/secret.py").is_err());
    assert!(normalize_strategy_file_name("runtime\\secret.py").is_err());
}

#[test]
fn discoverable_strategy_files_returns_empty_when_strategies_dir_is_missing() {
    let root = temp_project_root("discoverable_strategy_files_missing_dir");

    let files = discoverable_strategy_files(&root).unwrap();

    assert!(files.is_empty());
    assert!(!root.join("strategies").exists());
    fs::remove_dir_all(root).ok();
}

#[test]
fn discoverable_strategy_files_include_runtime_and_exclude_archive() {
    let root = temp_project_root("discoverable_strategy_files_include_runtime");
    let strategies = root.join("strategies");
    fs::create_dir_all(strategies.join("runtime")).unwrap();
    fs::create_dir_all(strategies.join("archive")).unwrap();
    fs::write(strategies.join("root_strategy.py"), "").unwrap();
    fs::write(strategies.join("runtime").join("runtime_strategy.py"), "").unwrap();
    fs::write(strategies.join("archive").join("archived_strategy.py"), "").unwrap();

    let mut files = discoverable_strategy_files(&root).unwrap();
    files.sort();

    assert_eq!(
        files,
        vec!["root_strategy.py", "runtime/runtime_strategy.py"]
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn discoverable_strategy_files_ignore_removed_paper_directory() {
    let root = temp_project_root("discoverable_strategy_files_ignore_removed_paper");
    let strategies = root.join("strategies");
    fs::create_dir_all(strategies.join("runtime")).unwrap();
    fs::create_dir_all(strategies.join("paper")).unwrap();
    fs::write(strategies.join("runtime").join("runtime_strategy.py"), "").unwrap();
    fs::write(strategies.join("paper").join("old_paper_strategy.py"), "").unwrap();

    let files = discoverable_strategy_files(&root).unwrap();

    assert_eq!(files, vec!["runtime/runtime_strategy.py"]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn discoverable_strategy_ids_fast_reads_literal_ids_without_python() {
    let root = temp_project_root("discoverable_strategy_ids_fast");
    let strategies = root.join("strategies").join("runtime");
    fs::create_dir_all(&strategies).unwrap();
    fs::write(
        strategies.join("one.py"),
        r#"
STRATEGY_ID = "fast_strategy_one"
STRATEGY_NAME = "Fast Strategy One"
"#,
    )
    .unwrap();
    fs::write(
        strategies.join("two.py"),
        r#"
STRATEGY_ID = 'fast_strategy_two'
raise RuntimeError("fast metadata scan must not execute this file")
"#,
    )
    .unwrap();

    let ids = discoverable_strategy_ids_fast(&root).unwrap();

    assert_eq!(ids, vec!["fast_strategy_one", "fast_strategy_two"]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn discover_runtime_strategy_requires_runtime_metadata_contract() {
    let root = temp_project_root("discover_runtime_strategy_requires_runtime_metadata_contract");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        "runtime/incomplete_strategy.py",
        r#"
STRATEGY_ID = "incomplete_strategy"
STRATEGY_NAME = "Incomplete Strategy"

def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}}
"#,
    );

    let error = discover_runtime_strategy(&root, "runtime/incomplete_strategy.py")
        .unwrap_err()
        .to_string();

    assert!(error.contains("RUNTIME_CONFIG 必须定义为 dict"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn discover_runtime_strategy_loads_runtime_metadata() {
    let root = temp_project_root("discover_runtime_strategy_loads_runtime_metadata");
    prepare_python_runner(&root);
    let source = runtime_strategy_source(
        "runtime_fixture_strategy",
        "Runtime Fixture Strategy",
        r#"
def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}}
"#,
    );
    write_runtime_strategy(&root, "runtime/runtime_fixture_strategy.py", &source);

    let meta = discover_runtime_strategy(&root, "runtime/runtime_fixture_strategy.py").unwrap();

    assert_eq!(meta.strategy_id, "runtime_fixture_strategy");
    assert_eq!(meta.strategy_name, "Runtime Fixture Strategy");
    assert_eq!(meta.file_name, "runtime/runtime_fixture_strategy.py");
    assert_eq!(meta.strategy_type, "single_symbol_strategy");
    assert!(meta.data_requirements["candles"].is_array());
    assert_eq!(meta.runtime_config["timeframe"].as_str(), Some("15m"));
    assert!(meta.visualization["indicator_series"].is_array());
    assert!(meta.decision_contract["entry_sides"].is_array());
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_execution_stamp_fingerprints_strategy_and_runner_files() {
    let root = temp_project_root("runtime_execution_stamp_fingerprints_files");
    prepare_python_runner(&root);
    let source = runtime_strategy_source(
        "runtime_stamp_strategy",
        "Runtime Stamp Strategy",
        r#"
def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}}
"#,
    );
    write_runtime_strategy(&root, "runtime/runtime_stamp_strategy.py", &source);

    let stamp = runtime_execution_stamp(&root, "runtime/runtime_stamp_strategy.py");

    assert_eq!(stamp["schema"].as_str(), Some("runtime_execution_stamp_v1"));
    assert_eq!(
        stamp["strategy"]["project_relative_path"].as_str(),
        Some("strategies/runtime/runtime_stamp_strategy.py")
    );
    assert_eq!(stamp["strategy"]["exists"].as_bool(), Some(true));
    assert!(stamp["strategy"]["len"].as_u64().unwrap_or(0) > 0);
    assert_eq!(stamp["strategy"]["sha256"].as_str().unwrap_or("").len(), 64);
    assert_eq!(
        stamp["runner"]["project_relative_path"].as_str(),
        Some("src-tauri/python/strategy_runner.py")
    );
    assert_eq!(stamp["runner"]["exists"].as_bool(), Some(true));
    assert_eq!(stamp["runner"]["sha256"].as_str().unwrap_or("").len(), 64);
    fs::remove_dir_all(root).ok();
}
