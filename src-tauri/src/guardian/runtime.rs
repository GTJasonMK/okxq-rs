use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::time::now_text;

#[derive(Clone, Debug, Default, Serialize)]
pub struct GuardianRuntimeSnapshot {
    pub active: bool,
    pub current_inst_id: String,
    pub current_timeframe: String,
    pub current_mode: String,
    pub current_phase: String,
    pub cycle_completed_units: i64,
    pub cycle_total_units: i64,
    pub last_run_started_at: Option<String>,
    pub last_run_finished_at: Option<String>,
    pub last_successful_run_at: Option<String>,
    pub last_triggered_at: Option<String>,
    pub last_run_summary: Value,
    pub last_sync_results: Vec<Value>,
    pub last_errors: Vec<Value>,
}

#[derive(Clone, Default)]
pub struct GuardianRuntime {
    inner: Arc<Mutex<GuardianRuntimeSnapshot>>,
}

impl GuardianRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start_cycle(&self, total_units: i64) {
        let now = now_text();
        let mut inner = self.inner.lock().await;
        inner.active = true;
        inner.current_phase = "scheduling".to_string();
        inner.current_inst_id.clear();
        inner.current_timeframe.clear();
        inner.current_mode.clear();
        inner.cycle_completed_units = 0;
        inner.cycle_total_units = total_units.max(0);
        inner.last_triggered_at = Some(now.clone());
        inner.last_run_started_at = Some(now);
        inner.last_run_finished_at = None;
        inner.last_sync_results.clear();
        inner.last_errors.clear();
        inner.last_run_summary =
            json!({"success_count": 0, "error_count": 0, "total_units": total_units.max(0)});
    }

    pub async fn record_scheduled(
        &self,
        inst_id: &str,
        timeframe: &str,
        mode: &str,
        completed_units: i64,
    ) {
        let mut inner = self.inner.lock().await;
        inner.current_inst_id = inst_id.to_string();
        inner.current_timeframe = timeframe.to_string();
        inner.current_mode = mode.to_string();
        inner.current_phase = "queued".to_string();
        inner.cycle_completed_units = completed_units.max(0);
    }

    pub async fn finish_cycle(&self, results: Vec<Value>, errors: Vec<Value>) {
        let now = now_text();
        let mut inner = self.inner.lock().await;
        inner.active = false;
        inner.current_phase = "idle".to_string();
        inner.current_inst_id.clear();
        inner.current_timeframe.clear();
        inner.current_mode.clear();
        inner.cycle_completed_units = inner.cycle_total_units;
        inner.last_run_finished_at = Some(now.clone());
        if errors.is_empty() {
            inner.last_successful_run_at = Some(now);
        }
        inner.last_run_summary = json!({
            "success_count": results.len(),
            "error_count": errors.len(),
            "total_units": inner.cycle_total_units
        });
        inner.last_sync_results = results;
        inner.last_errors = errors;
    }

    pub async fn snapshot(&self) -> GuardianRuntimeSnapshot {
        self.inner.lock().await.clone()
    }
}
