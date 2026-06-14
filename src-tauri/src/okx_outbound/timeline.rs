use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use serde_json::{json, Value};

use super::OKXOutboundEvent;

/// 出站事件时间线存储，记录最近的 API 调用事件
pub struct OKXOutboundTimelineStore {
    events: Mutex<VecDeque<OKXOutboundEvent>>,
    max_events: usize,
}

impl OKXOutboundTimelineStore {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(max_events)),
            max_events,
        }
    }

    /// 记录一次出站事件
    pub fn record(&self, event: OKXOutboundEvent) {
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// 生成时间窗口快照
    pub fn snapshot(&self, window_seconds: i64, now_ts: f64, limit: usize) -> Value {
        let lower_bound = now_ts - (window_seconds.max(1) as f64);
        let normalized_limit = limit.max(1);
        let events = self.events.lock().unwrap_or_else(|e| e.into_inner());

        let window_events: Vec<&OKXOutboundEvent> =
            events.iter().filter(|e| e.ts >= lower_bound).collect();

        let total = window_events.len();
        let sliced: Vec<&OKXOutboundEvent> = if total > normalized_limit {
            window_events[total - normalized_limit..].to_vec()
        } else {
            window_events
        };

        let event_values: Vec<Value> = sliced
            .iter()
            .map(|e| {
                json!({
                    "ts": e.ts,
                    "op_key": e.op_key,
                    "channel": e.channel,
                    "target_group": e.target_group,
                    "rule_key": e.rule_key,
                    "scope_key": e.scope_key,
                    "inst_id": e.inst_id,
                    "mode": e.mode,
                    "result": e.result,
                    "latency_ms": e.latency_ms,
                })
            })
            .collect();

        // 统计操作频次
        let mut op_counts: BTreeMap<&str, usize> = BTreeMap::new();
        let mut error_count = 0;
        let mut success_count = 0;
        for e in &sliced {
            *op_counts.entry(e.op_key.as_str()).or_insert(0) += 1;
            if e.result == "ok" {
                success_count += 1;
            } else {
                error_count += 1;
            }
        }

        let mut top_operations: Vec<_> = op_counts.iter().collect();
        top_operations.sort_by(|a, b| b.1.cmp(a.1));
        let top_ops: Vec<Value> = top_operations
            .into_iter()
            .take(5)
            .map(|(op_key, count)| json!({"op_key": op_key, "count": count}))
            .collect();

        // 最慢操作
        let mut slowest: Vec<_> = sliced.iter().collect();
        slowest.sort_by(|a, b| {
            b.latency_ms
                .cmp(&a.latency_ms)
                .then_with(|| b.ts.partial_cmp(&a.ts).unwrap_or(std::cmp::Ordering::Equal))
        });
        let slowest_ops: Vec<Value> = slowest
            .into_iter()
            .take(5)
            .map(|e| json!({"op_key": e.op_key, "latency_ms": e.latency_ms}))
            .collect();

        json!({
            "window_seconds": window_seconds.max(1),
            "generated_at": now_ts,
            "events": event_values,
            "summary": {
                "event_count": sliced.len(),
                "error_count": error_count,
                "success_count": success_count,
                "top_operations": top_ops,
                "slowest_operations": slowest_ops,
            }
        })
    }
}

impl Default for OKXOutboundTimelineStore {
    fn default() -> Self {
        Self::new(12000)
    }
}
