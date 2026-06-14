use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::Serialize;
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Serialize)]
pub struct BacktestProgress {
    pub run_id: String,
    pub strategy_id: String,
    pub status: String,
    pub stage: String,
    pub message: String,
    pub progress: i64,
    pub processed_candles: i64,
    pub total_candles: i64,
    pub strategy_progress: Value,
    pub started_at: String,
    pub updated_at: String,
}

struct BacktestProgressUpdate<'a> {
    progress: i64,
    stage: &'a str,
    message: &'a str,
    processed_candles: usize,
    total_candles: usize,
    strategy_progress: Value,
}

#[derive(Clone, Default)]
pub struct BacktestProgressRegistry {
    inner: Arc<RwLock<HashMap<String, BacktestProgress>>>,
}

#[derive(Clone, Default)]
pub struct BacktestProgressReporter {
    registry: Option<BacktestProgressRegistry>,
    run_id: String,
    strategy_id: String,
}

impl BacktestProgressRegistry {
    fn start(&self, run_id: &str, strategy_id: &str, message: &str) {
        let now = now_text();
        let progress = BacktestProgress {
            run_id: run_id.to_string(),
            strategy_id: strategy_id.to_string(),
            status: "running".to_string(),
            stage: "prepare".to_string(),
            message: message.to_string(),
            progress: 1,
            processed_candles: 0,
            total_candles: 0,
            strategy_progress: Value::Null,
            started_at: now.clone(),
            updated_at: now,
        };
        let mut items = self
            .inner
            .write()
            .expect("backtest progress registry write lock should not be poisoned");
        items.insert(run_id.to_string(), progress);
    }

    fn report(&self, run_id: &str, update: BacktestProgressUpdate<'_>) {
        let mut items = self
            .inner
            .write()
            .expect("backtest progress registry write lock should not be poisoned");
        let now = now_text();
        let item = items
            .get_mut(run_id)
            .expect("backtest progress should be started before report");
        item.status = "running".to_string();
        item.stage = update.stage.to_string();
        item.message = update.message.to_string();
        item.progress = item.progress.max(clamp_percent(update.progress));
        item.processed_candles = update.processed_candles as i64;
        item.total_candles = update.total_candles as i64;
        item.strategy_progress = update.strategy_progress;
        item.updated_at = now;
    }

    fn complete(&self, run_id: &str, message: &str) {
        self.finish(run_id, "completed", 100, "complete", message);
    }

    fn fail(&self, run_id: &str, message: &str) {
        self.finish(run_id, "failed", 100, "failed", message);
    }

    pub fn get(&self, run_id: &str) -> Option<BacktestProgress> {
        let items = self
            .inner
            .read()
            .expect("backtest progress registry read lock should not be poisoned");
        items.get(run_id).cloned()
    }

    fn finish(&self, run_id: &str, status: &str, progress: i64, stage: &str, message: &str) {
        let mut items = self
            .inner
            .write()
            .expect("backtest progress registry write lock should not be poisoned");
        let now = now_text();
        let item = items
            .get_mut(run_id)
            .expect("backtest progress should be started before finish");
        item.status = status.to_string();
        item.stage = stage.to_string();
        item.message = message.to_string();
        item.progress = clamp_percent(progress);
        item.updated_at = now;
    }
}

impl BacktestProgressReporter {
    pub fn new(registry: BacktestProgressRegistry, run_id: String, strategy_id: &str) -> Self {
        if run_id.trim().is_empty() {
            return Self::default();
        }
        Self {
            registry: Some(registry),
            run_id,
            strategy_id: strategy_id.to_string(),
        }
    }

    pub fn start(&self, message: &str) {
        if let Some(registry) = &self.registry {
            registry.start(&self.run_id, &self.strategy_id, message);
        }
    }

    pub fn report_detail(
        &self,
        progress: i64,
        stage: &str,
        message: &str,
        processed_candles: usize,
        total_candles: usize,
        detail: Value,
    ) {
        self.report_with_strategy_progress(
            progress,
            stage,
            message,
            processed_candles,
            total_candles,
            progress_detail(stage, message, processed_candles, total_candles, detail),
        );
    }

    pub fn report_strategy_step_with_detail(
        &self,
        processed_candles: usize,
        total_candles: usize,
        diagnostics: &Value,
        detail: Value,
    ) {
        assert!(
            total_candles > 0,
            "backtest strategy progress total should be positive"
        );
        assert!(
            processed_candles <= total_candles,
            "backtest strategy progress processed candles should not exceed total"
        );
        let strategy_progress = explicit_strategy_progress(diagnostics);
        let strategy_fraction = strategy_progress
            .as_ref()
            .and_then(progress_fraction_from_value);
        let loop_fraction = processed_candles as f64 / total_candles as f64;
        let progress = backtest_strategy_stage_percent(strategy_fraction.unwrap_or(loop_fraction));
        let stage = strategy_progress
            .as_ref()
            .and_then(|item| string_field(item, "stage"))
            .unwrap_or_else(|| "strategy".to_string());
        let message = strategy_progress
            .as_ref()
            .and_then(|item| string_field(item, "message"))
            .unwrap_or_else(|| format!("执行策略 {processed_candles}/{total_candles} 根K线"));
        let detail = progress_detail(
            &stage,
            &message,
            processed_candles,
            total_candles,
            merge_progress_values(strategy_progress.unwrap_or(Value::Null), detail),
        );

        self.report_with_strategy_progress(
            progress,
            &stage,
            &message,
            processed_candles,
            total_candles,
            detail,
        );
    }

    pub fn report_python_progress(&self, event: &Value) {
        let strategy_progress = event
            .get("strategy_progress")
            .cloned()
            .unwrap_or_else(|| event.clone());
        let fraction = progress_fraction_from_value(event)
            .expect("python progress event should include progress");
        let progress = backtest_strategy_stage_percent(fraction);
        let stage = string_field(event, "stage").unwrap_or_else(|| "strategy".to_string());
        let message = string_field(event, "message").unwrap_or_else(|| "执行策略".to_string());
        let processed_candles = usize_field(event, "processed");
        let total_candles = usize_field(event, "total");
        let strategy_progress = progress_detail(
            &stage,
            &message,
            processed_candles,
            total_candles,
            strategy_progress,
        );
        self.report_with_strategy_progress(
            progress,
            &stage,
            &message,
            processed_candles,
            total_candles,
            strategy_progress,
        );
    }

    pub fn complete(&self, message: &str) {
        if let Some(registry) = &self.registry {
            registry.complete(&self.run_id, message);
        }
    }

    pub fn fail(&self, message: &str) {
        if let Some(registry) = &self.registry {
            registry.fail(&self.run_id, message);
        }
    }

    fn report_with_strategy_progress(
        &self,
        progress: i64,
        stage: &str,
        message: &str,
        processed_candles: usize,
        total_candles: usize,
        strategy_progress: Value,
    ) {
        if let Some(registry) = &self.registry {
            registry.report(
                &self.run_id,
                BacktestProgressUpdate {
                    progress,
                    stage,
                    message,
                    processed_candles,
                    total_candles,
                    strategy_progress,
                },
            );
        }
    }
}

pub fn idle_progress(run_id: &str) -> BacktestProgress {
    let now = now_text();
    BacktestProgress {
        run_id: run_id.to_string(),
        strategy_id: String::new(),
        status: "idle".to_string(),
        stage: "idle".to_string(),
        message: "等待运行".to_string(),
        progress: 0,
        processed_candles: 0,
        total_candles: 0,
        strategy_progress: Value::Null,
        started_at: now.clone(),
        updated_at: now,
    }
}

fn explicit_strategy_progress(diagnostics: &Value) -> Option<Value> {
    for key in ["backtest_progress", "task_progress", "run_progress"] {
        if let Some(value) = diagnostics.get(key) {
            return Some(value.clone());
        }
    }
    None
}

fn progress_fraction_from_value(value: &Value) -> Option<f64> {
    let raw = value.get("progress").and_then(Value::as_f64)?;
    if !raw.is_finite() {
        return None;
    }
    Some(if raw > 1.0 { raw / 100.0 } else { raw }.clamp(0.0, 1.0))
}

pub(crate) fn backtest_strategy_stage_percent(fraction: f64) -> i64 {
    (35.0 + fraction.clamp(0.0, 1.0) * 55.0).round() as i64
}

fn progress_detail(
    stage: &str,
    message: &str,
    processed_candles: usize,
    total_candles: usize,
    detail: Value,
) -> Value {
    let mut object = match detail {
        Value::Object(object) => object,
        Value::Null => Map::new(),
        other => {
            let mut object = Map::new();
            object.insert("detail".to_string(), other);
            object
        }
    };
    insert_if_missing(&mut object, "stage", json!(stage));
    insert_if_missing(&mut object, "message", json!(message));
    insert_if_missing(&mut object, "processed_candles", json!(processed_candles));
    insert_if_missing(&mut object, "total_candles", json!(total_candles));
    if total_candles > 0 {
        insert_if_missing(
            &mut object,
            "progress",
            json!((processed_candles as f64 / total_candles as f64).clamp(0.0, 1.0)),
        );
    }
    Value::Object(object)
}

fn merge_progress_values(progress: Value, detail: Value) -> Value {
    match (progress, detail) {
        (Value::Object(mut progress), Value::Object(detail)) => {
            for (key, value) in detail {
                progress.entry(key).or_insert(value);
            }
            Value::Object(progress)
        }
        (Value::Null, detail) => detail,
        (progress, Value::Null) => progress,
        (progress, Value::Object(mut detail)) => {
            detail.insert("strategy_progress".to_string(), progress);
            Value::Object(detail)
        }
        (progress, detail) => json!({
            "strategy_progress": progress,
            "detail": detail,
        }),
    }
}

fn insert_if_missing(object: &mut Map<String, Value>, key: &str, value: Value) {
    if !object.contains_key(key) {
        object.insert(key.to_string(), value);
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn usize_field(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_i64)
        .unwrap_or_default()
        .max(0) as usize
}

fn clamp_percent(value: i64) -> i64 {
    value.clamp(0, 100)
}

fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        backtest_strategy_stage_percent, BacktestProgressRegistry, BacktestProgressReporter,
        BacktestProgressUpdate,
    };

    #[test]
    fn backtest_strategy_stage_percent_maps_strategy_fraction_to_runtime_band() {
        assert_eq!(backtest_strategy_stage_percent(0.0), 35);
        assert_eq!(backtest_strategy_stage_percent(0.5), 63);
        assert_eq!(backtest_strategy_stage_percent(1.0), 90);
    }

    #[test]
    fn backtest_strategy_stage_percent_clamps_out_of_range_fraction() {
        assert_eq!(backtest_strategy_stage_percent(-1.0), 35);
        assert_eq!(backtest_strategy_stage_percent(2.0), 90);
    }

    #[test]
    fn progress_reporter_updates_started_run() {
        let registry = BacktestProgressRegistry::default();
        let reporter =
            BacktestProgressReporter::new(registry.clone(), "run-1".to_string(), "strategy-a");

        reporter.start("准备运行策略");
        reporter.report_detail(
            12,
            "config",
            "策略配置已加载",
            2,
            5,
            json!({ "step": "strategy_config_ready" }),
        );

        let progress = registry.get("run-1").expect("progress should be stored");
        assert_eq!(progress.run_id, "run-1");
        assert_eq!(progress.strategy_id, "strategy-a");
        assert_eq!(progress.status, "running");
        assert_eq!(progress.stage, "config");
        assert_eq!(progress.message, "策略配置已加载");
        assert_eq!(progress.progress, 12);
        assert_eq!(progress.processed_candles, 2);
        assert_eq!(progress.total_candles, 5);
        assert_eq!(
            progress.strategy_progress["step"],
            json!("strategy_config_ready")
        );

        reporter.complete("回测完成");
        let progress = registry.get("run-1").expect("progress should be stored");
        assert_eq!(progress.status, "completed");
        assert_eq!(progress.stage, "complete");
        assert_eq!(progress.progress, 100);
    }

    #[test]
    fn strategy_progress_ignores_removed_alias_fields() {
        let registry = BacktestProgressRegistry::default();
        let reporter =
            BacktestProgressReporter::new(registry.clone(), "run-1".to_string(), "strategy-a");

        reporter.start("准备运行策略");
        reporter.report_strategy_step_with_detail(
            5,
            10,
            &json!({
                "backtest_progress": {
                    "pct": 0.9,
                    "phase": "legacy_phase",
                    "summary": "legacy summary"
                }
            }),
            json!({}),
        );

        let progress = registry.get("run-1").expect("progress should be stored");
        assert_eq!(progress.progress, 63);
        assert_eq!(progress.stage, "strategy");
        assert_eq!(progress.message, "执行策略 5/10 根K线");
    }

    #[test]
    fn python_progress_event_uses_canonical_fields() {
        let registry = BacktestProgressRegistry::default();
        let reporter =
            BacktestProgressReporter::new(registry.clone(), "run-1".to_string(), "strategy-a");

        reporter.start("准备运行策略");
        reporter.report_python_progress(&json!({
            "event": "progress",
            "progress": 0.25,
            "stage": "candidate_selection",
            "message": "Selecting candidates",
            "processed": 3,
            "total": 12,
            "strategy_progress": {
                "progress": 0.25,
                "stage": "candidate_selection",
                "message": "Selecting candidates"
            }
        }));

        let progress = registry.get("run-1").expect("progress should be stored");
        assert_eq!(progress.progress, 49);
        assert_eq!(progress.stage, "candidate_selection");
        assert_eq!(progress.message, "Selecting candidates");
        assert_eq!(progress.processed_candles, 3);
        assert_eq!(progress.total_candles, 12);
    }

    #[test]
    #[should_panic(expected = "python progress event should include progress")]
    fn python_progress_event_requires_canonical_progress() {
        let registry = BacktestProgressRegistry::default();
        let reporter =
            BacktestProgressReporter::new(registry.clone(), "run-1".to_string(), "strategy-a");

        reporter.start("准备运行策略");
        reporter.report_python_progress(&json!({
            "event": "progress",
            "pct": 0.25,
            "stage": "candidate_selection",
            "message": "Selecting candidates"
        }));
    }

    #[test]
    fn empty_progress_id_disables_reporter() {
        let registry = BacktestProgressRegistry::default();
        let reporter =
            BacktestProgressReporter::new(registry.clone(), " ".to_string(), "strategy-a");

        reporter.start("准备运行策略");
        reporter.report_detail(12, "config", "策略配置已加载", 2, 5, json!({}));
        reporter.complete("回测完成");

        assert!(registry.get(" ").is_none());
    }

    #[test]
    #[should_panic(expected = "backtest progress should be started before report")]
    fn registry_report_requires_started_run() {
        let registry = BacktestProgressRegistry::default();

        registry.report(
            "run-1",
            BacktestProgressUpdate {
                progress: 12,
                stage: "config",
                message: "策略配置已加载",
                processed_candles: 2,
                total_candles: 5,
                strategy_progress: json!({}),
            },
        );
    }
}
