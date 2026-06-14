use serde_json::Value;

use crate::strategy_executor::types::RuntimeStrategyExecutionLog;

use super::{storage::insert_live_execution_log, LiveExecutionLogEntry, LiveStrategyRuntime};

const MAX_EXECUTION_LOGS: usize = 300;

#[derive(Clone, Debug, Default)]
struct ExecutionLogScope {
    mode: String,
    strategy_id: String,
    strategy_name: String,
    symbol: String,
    inst_type: String,
    timeframe: String,
}

impl LiveStrategyRuntime {
    #[cfg(test)]
    pub async fn execution_logs(&self, run_id: &str, limit: usize) -> Vec<LiveExecutionLogEntry> {
        let run_id = run_id.trim();
        let limit = limit.clamp(1, MAX_EXECUTION_LOGS);
        let logs = self.inner.execution_logs.lock().await;
        let mut rows = logs
            .iter()
            .rev()
            .filter(|entry| run_id.is_empty() || entry.run_id == run_id)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        rows.reverse();
        rows
    }

    pub(crate) async fn clear_execution_logs(&self) {
        {
            let mut logs = self.inner.execution_logs.lock().await;
            logs.clear();
        }
        let mut seq = self.inner.execution_log_seq.lock().await;
        *seq = 0;
    }

    pub(crate) async fn log_execution_stage(
        &self,
        run_id: &str,
        stage: &str,
        level: &str,
        message: impl Into<String>,
        details: Value,
    ) {
        if run_id.trim().is_empty() {
            return;
        }
        let seq = {
            let mut guard = self.inner.execution_log_seq.lock().await;
            *guard += 1;
            *guard
        };
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let scope = self.execution_log_scope(run_id, &details).await;
        let entry = LiveExecutionLogEntry {
            seq,
            run_id: run_id.to_string(),
            mode: scope.mode,
            strategy_id: scope.strategy_id,
            strategy_name: scope.strategy_name,
            symbol: scope.symbol,
            inst_type: scope.inst_type,
            timeframe: scope.timeframe,
            timestamp_ms,
            time: timestamp_iso(timestamp_ms),
            stage: stage.trim().to_string(),
            level: normalize_level(level),
            message: message.into(),
            details,
        };
        let mut logs = self.inner.execution_logs.lock().await;
        logs.push_back(entry);
        while logs.len() > MAX_EXECUTION_LOGS {
            logs.pop_front();
        }
        let persisted_entry = logs.back().cloned();
        drop(logs);
        if let Some(entry) = persisted_entry {
            self.persist_execution_log_entry(entry).await;
        }
    }

    pub(crate) async fn log_strategy_execution_entry(
        &self,
        run_id: &str,
        entry: &RuntimeStrategyExecutionLog,
    ) {
        let details = strategy_details_with_source(entry.details.clone(), "strategy_decision");
        self.log_execution_stage(
            run_id,
            &entry.stage,
            &entry.level,
            entry.message.clone(),
            details,
        )
        .await;
    }

    pub(crate) async fn log_strategy_event(&self, run_id: &str, event: &Value) {
        let event_name = event.get("event").and_then(Value::as_str).unwrap_or("");
        if !matches!(event_name, "progress" | "strategy_log") {
            return;
        }
        let stage = string_value(event, &["stage"], "strategy");
        let level = string_value(event, &["level"], "info");
        let message = string_value(
            event,
            &["message"],
            if event_name == "progress" {
                "策略内部进度更新"
            } else {
                "策略内部日志"
            },
        );
        self.log_execution_stage(
            run_id,
            &stage,
            &level,
            message,
            strategy_details_with_source(event.clone(), "strategy_event"),
        )
        .await;
    }
}

impl LiveStrategyRuntime {
    async fn execution_log_scope(&self, run_id: &str, details: &Value) -> ExecutionLogScope {
        let status = self.inner.status.read().await.clone();
        let status_matches = !run_id.trim().is_empty() && status.run_id == run_id.trim();
        ExecutionLogScope {
            mode: if status_matches {
                status.mode
            } else {
                string_value(details, &["mode"], "")
            },
            strategy_id: if status_matches {
                status.strategy_id
            } else {
                string_value(details, &["strategy_id"], "")
            },
            strategy_name: if status_matches {
                status.strategy_name
            } else {
                string_value(details, &["strategy_name"], "")
            },
            symbol: if status_matches {
                status.symbol
            } else {
                string_value(details, &["symbol"], "")
            },
            inst_type: if status_matches {
                status.inst_type
            } else {
                string_value(details, &["inst_type"], "")
            },
            timeframe: if status_matches {
                status.timeframe
            } else {
                string_value(details, &["timeframe"], "")
            },
        }
    }

    async fn persist_execution_log_entry(&self, entry: LiveExecutionLogEntry) {
        let pool = self.inner.execution_log_db.read().await.clone();
        let Some(pool) = pool else {
            return;
        };
        if let Err(error) = insert_live_execution_log(&pool, &entry).await {
            tracing::warn!(
                run_id = %entry.run_id,
                stage = %entry.stage,
                "持久化实时策略执行日志失败: {error}"
            );
        }
    }
}

fn string_value(value: &Value, keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|item| !item.trim().is_empty())
        .unwrap_or(default)
        .to_string()
}

fn strategy_details_with_source(details: Value, source: &str) -> Value {
    match details {
        Value::Object(mut object) => {
            object.insert("source".to_string(), Value::String(source.to_string()));
            Value::Object(object)
        }
        other => serde_json::json!({
            "source": source,
            "value": other,
        }),
    }
}

fn normalize_level(level: &str) -> String {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => "error".to_string(),
        "warn" | "warning" => "warn".to_string(),
        "success" => "success".to_string(),
        _ => "info".to_string(),
    }
}

fn timestamp_iso(timestamp_ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_ms)
        .expect("live execution log timestamp should be representable")
        .to_rfc3339()
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }

    #[tokio::test]
    async fn execution_logs_are_filtered_and_limited() {
        let runtime = LiveStrategyRuntime::new();
        runtime
            .log_execution_stage("run-a", "start", "info", "A1", json!({}))
            .await;
        runtime
            .log_execution_stage("run-b", "start", "warn", "B1", json!({}))
            .await;
        runtime
            .log_execution_stage("run-a", "done", "success", "A2", json!({}))
            .await;

        let rows = runtime.execution_logs("run-a", 10).await;

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].message, "A1");
        assert_eq!(rows[1].message, "A2");
        assert_eq!(rows[1].level, "success");
    }

    #[tokio::test]
    async fn clear_execution_logs_resets_sequence() {
        let runtime = LiveStrategyRuntime::new();
        runtime
            .log_execution_stage("run-a", "start", "info", "A1", json!({}))
            .await;
        runtime.clear_execution_logs().await;
        runtime
            .log_execution_stage("run-a", "start", "info", "A2", json!({}))
            .await;

        let rows = runtime.execution_logs("", 10).await;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seq, 1);
        assert_eq!(rows[0].message, "A2");
    }

    #[tokio::test]
    async fn execution_logs_are_persisted_when_runtime_has_database() {
        let db_path = temp_db_path("runtime_execution_logs_persist");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let runtime = LiveStrategyRuntime::new();
        {
            let mut log_db = runtime.inner.execution_log_db.write().await;
            *log_db = Some(pool.clone());
        }

        runtime
            .log_execution_stage(
                "run-persist",
                "submit",
                "success",
                "OKX 订单已提交",
                json!({
                    "mode": "simulated",
                    "strategy_id": "strategy-a",
                    "strategy_name": "Strategy A",
                    "symbol": "BTC-USDT-SWAP",
                    "inst_type": "SWAP",
                    "timeframe": "15m",
                    "order_id": "order-1"
                }),
            )
            .await;

        let persisted =
            crate::live_strategy::query_live_execution_logs(&pool, 10, "simulated", "run-persist")
                .await
                .expect("persisted logs should query");

        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].stage, "submit");
        assert_eq!(persisted[0].level, "success");
        assert_eq!(persisted[0].mode, "simulated");
        assert_eq!(persisted[0].strategy_id, "strategy-a");
        assert_eq!(persisted[0].symbol, "BTC-USDT-SWAP");
        assert_eq!(persisted[0].details["order_id"], json!("order-1"));

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}
