use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) static TEST_REGISTRY_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn repo_project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub(super) fn temp_project_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
}

pub(super) fn prepare_python_runner(root: &Path) {
    let runner_dir = root.join("src-tauri").join("python");
    fs::create_dir_all(&runner_dir).unwrap();
    let source_dir = repo_project_root().join("src-tauri/python");
    fs::copy(
        source_dir.join("strategy_runner.py"),
        runner_dir.join("strategy_runner.py"),
    )
    .unwrap();
    for entry in fs::read_dir(source_dir).unwrap() {
        let path = entry.unwrap().path();
        let Some(file_name) = path.file_name().and_then(|item| item.to_str()) else {
            continue;
        };
        if !file_name.starts_with("strategy_runner_") || !file_name.ends_with(".py") {
            continue;
        }
        fs::copy(&path, runner_dir.join(file_name)).unwrap();
    }
}

pub(super) fn write_runtime_strategy(root: &Path, file_name: &str, source: &str) -> PathBuf {
    let path = root.join("strategies").join(file_name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, source).unwrap();
    path
}

pub(super) fn runtime_strategy_source(
    strategy_id: &str,
    strategy_name: &str,
    body: &str,
) -> String {
    format!(
        r#"
STRATEGY_ID = "{strategy_id}"
STRATEGY_NAME = "{strategy_name}"
STRATEGY_DESCRIPTION = "Runtime metadata fixture"
STRATEGY_TYPE = "single_symbol_strategy"
RUNTIME_CONFIG = {{
    "symbol": "BTC-USDT-SWAP",
    "inst_type": "SWAP",
    "timeframe": "15m",
    "risk_timeframe": "1m",
    "initial_capital": 1000,
    "position_size": 0.2,
    "stop_loss": 0.03,
    "take_profit": 0.06,
    "check_interval": 60,
    "params": {{"runtime_only": True}},
}}
DATA_REQUIREMENTS = {{
    "candles": [
        {{
            "role": "primary",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m",
            "min_bars": 3,
        }}
    ],
    "funding": [],
    "orderbook": [],
}}
VISUALIZATION = {{
    "primary_price_series": "close",
    "indicator_series": [{{"key": "close", "label": "Close", "unit": "price"}}],
    "diagnostics": ["close"],
}}
DECISION_CONTRACT = {{
    "entry_sides": ["buy", "sell"],
    "exit_sides": ["flat"],
    "hold_sides": ["hold"],
    "reason_codes": ["fixture_signal"],
}}

{body}
"#
    )
}

pub(super) fn rising_candles(start_ts: i64, minutes: i64, count: usize) -> Vec<Value> {
    let mut price = 100.0;
    (0..count)
        .map(|index| {
            price *= 1.01;
            json!({
                "timestamp": start_ts + index as i64 * minutes * 60_000,
                "open": price * 0.99,
                "high": price * 1.02,
                "low": price * 0.98,
                "close": price,
                "volume": 1.0
            })
        })
        .collect()
}
