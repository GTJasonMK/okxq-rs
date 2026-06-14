use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::TokenBucketRegistry;

impl TokenBucketRegistry {
    /// Diagnostics for the UI and console debugging. This exposes both the
    /// official rolling windows and the proactive in-flight concurrency gates.
    pub async fn snapshot(&self) -> Value {
        let window_values = {
            let now = Instant::now();
            let mut windows = self.windows.lock().await;
            let mut values = Vec::with_capacity(windows.len());

            for (bucket_key, state) in windows.iter_mut() {
                let op_key = bucket_key.split('|').next().unwrap_or(bucket_key.as_str());
                let Some(rule) = self.rules.get(op_key) else {
                    values.push(json!({
                        "bucket_key": bucket_key,
                        "op_key": op_key,
                        "calls_in_window": state.calls.len(),
                    }));
                    continue;
                };
                let window = Duration::from_secs(rule.window_seconds as u64);
                while state
                    .calls
                    .front()
                    .is_some_and(|oldest| now.duration_since(*oldest) >= window)
                {
                    state.calls.pop_front();
                }
                values.push(json!({
                    "bucket_key": bucket_key,
                    "op_key": rule.op_key,
                    "rule_key": rule.rule_key,
                    "channel": rule.channel,
                    "target_group": rule.target_group,
                    "capacity": rule.capacity,
                    "window_seconds": rule.window_seconds,
                    "calls_in_window": state.calls.len(),
                    "remaining": (rule.capacity as usize).saturating_sub(state.calls.len()),
                }));
            }

            values.sort_by(|a, b| {
                a.get("bucket_key")
                    .and_then(Value::as_str)
                    .cmp(&b.get("bucket_key").and_then(Value::as_str))
            });
            values
        };

        let mut concurrency_values = self
            .concurrency
            .iter()
            .map(|(key, limiter)| {
                json!({
                    "group_key": key,
                    "capacity": limiter.capacity(),
                    "in_flight": limiter.in_flight(),
                    "available": limiter.available(),
                    "waiting": limiter.waiting(),
                })
            })
            .collect::<Vec<_>>();
        concurrency_values.sort_by(|a, b| {
            a.get("group_key")
                .and_then(Value::as_str)
                .cmp(&b.get("group_key").and_then(Value::as_str))
        });

        json!({
            "windows": window_values,
            "concurrency": concurrency_values,
        })
    }
}
