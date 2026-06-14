use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::okx_outbound::OKXOutboundRule;

use super::{
    concurrency::{default_concurrency_limiters, ConcurrencyLimiter, ConcurrencyPermit},
    settings::OKXConcurrencySettings,
    window::{bucket_key, RateWindowRule, RateWindowState},
    GLOBAL_CONCURRENCY_KEY, UNKNOWN_CONCURRENCY_KEY,
};

mod snapshot;

const WAIT_GRACE_SECONDS: f64 = 0.02;

/// Keeps one or more concurrency permits alive until the outbound operation ends.
pub struct OKXRequestPermit {
    _group: Option<ConcurrencyPermit>,
    _global: Option<ConcurrencyPermit>,
}

/// Shared OKX rate-limit governor.
pub struct TokenBucketRegistry {
    pub(super) rules: HashMap<String, RateWindowRule>,
    pub(super) windows: Mutex<HashMap<String, RateWindowState>>,
    pub(super) concurrency: HashMap<String, ConcurrencyLimiter>,
}

impl TokenBucketRegistry {
    /// Build the governor from OKX official REST/WS rules.
    pub fn from_rules(rules: &HashMap<String, OKXOutboundRule>) -> Self {
        Self {
            rules: rules
                .iter()
                .map(|(key, rule)| (key.clone(), RateWindowRule::from(rule)))
                .collect(),
            windows: Mutex::new(HashMap::new()),
            concurrency: default_concurrency_limiters(),
        }
    }

    pub fn apply_concurrency_settings(&self, settings: &OKXConcurrencySettings) {
        let settings = settings.clone().normalized();
        self.set_concurrency_limit(GLOBAL_CONCURRENCY_KEY, settings.okx_max_concurrency);
        self.set_concurrency_limit("rest:public", settings.okx_public_rest_concurrency);
        self.set_concurrency_limit("rest:private", settings.okx_private_rest_concurrency);
        self.set_concurrency_limit("rest:trade", settings.okx_trade_rest_concurrency);
        self.set_concurrency_limit("ws:ws_control", settings.okx_ws_control_concurrency);
        self.set_concurrency_limit(UNKNOWN_CONCURRENCY_KEY, settings.okx_unknown_concurrency);
    }

    /// Acquire permission for an operation. Unknown operations are allowed so
    /// experimental endpoints do not deadlock during development.
    pub async fn acquire(&self, op_key: &str, inst_id: Option<&str>) {
        let Some(rule) = self.rules.get(op_key).cloned() else {
            return;
        };
        let bucket_key = bucket_key(&rule, inst_id);
        let window = Duration::from_secs(rule.window_seconds as u64);

        loop {
            let wait_duration = {
                let now = Instant::now();
                let mut windows = self.windows.lock().await;
                let state = windows.entry(bucket_key.clone()).or_default();
                while state
                    .calls
                    .front()
                    .is_some_and(|oldest| now.duration_since(*oldest) >= window)
                {
                    state.calls.pop_front();
                }

                if state.calls.len() < rule.capacity as usize {
                    state.calls.push_back(now);
                    None
                } else {
                    let oldest = state.calls.front().copied().unwrap_or(now);
                    let elapsed = now.duration_since(oldest);
                    let wait = window.saturating_sub(elapsed)
                        + Duration::from_secs_f64(WAIT_GRACE_SECONDS);
                    tracing::debug!(
                        op_key = %rule.op_key,
                        rule_key = %rule.rule_key,
                        bucket_key = %bucket_key,
                        capacity = rule.capacity,
                        window_seconds = rule.window_seconds,
                        wait_ms = wait.as_millis() as i64,
                        "OKX outbound window full; waiting before request"
                    );
                    Some(wait)
                }
            };

            match wait_duration {
                Some(duration) if !duration.is_zero() => sleep(duration).await,
                _ => return,
            }
        }
    }

    /// Acquire an in-flight concurrency slot before the actual OKX send/connect.
    ///
    /// This is intentionally separate from the sliding window: callers should
    /// wait for the rate window first, then acquire this short-lived permit
    /// immediately before the outbound operation.
    pub async fn acquire_request_permit(
        &self,
        op_key: &str,
        inst_id: Option<&str>,
    ) -> OKXRequestPermit {
        let group_key = self.concurrency_group(op_key);
        let group = self
            .acquire_limiter(&group_key, op_key, inst_id, "target_group")
            .await;
        let global = self
            .acquire_limiter(GLOBAL_CONCURRENCY_KEY, op_key, inst_id, "global")
            .await;

        OKXRequestPermit {
            _group: group,
            _global: global,
        }
    }

    pub(super) async fn acquire_limiter(
        &self,
        limiter_key: &str,
        op_key: &str,
        inst_id: Option<&str>,
        limiter_scope: &str,
    ) -> Option<ConcurrencyPermit> {
        let limiter = self.concurrency.get(limiter_key).cloned()?;

        let waiting = limiter.available() == 0;
        let wait_start = Instant::now();
        if waiting {
            tracing::debug!(
                op_key,
                inst_id = inst_id.unwrap_or(""),
                limiter_key,
                limiter_scope,
                capacity = limiter.capacity(),
                in_flight = limiter.in_flight(),
                waiting = limiter.waiting(),
                "OKX outbound concurrency full; waiting before request"
            );
        }

        let permit = limiter.acquire().await;
        if waiting {
            tracing::debug!(
                op_key,
                inst_id = inst_id.unwrap_or(""),
                limiter_key,
                limiter_scope,
                waited_ms = wait_start.elapsed().as_millis() as i64,
                in_flight = limiter.in_flight(),
                waiting = limiter.waiting(),
                "OKX outbound concurrency permit acquired"
            );
        }
        Some(permit)
    }

    fn concurrency_group(&self, op_key: &str) -> String {
        self.rules
            .get(op_key)
            .map(|rule| format!("{}:{}", rule.channel, rule.target_group))
            .filter(|key| self.concurrency.contains_key(key))
            .unwrap_or_else(|| UNKNOWN_CONCURRENCY_KEY.to_string())
    }

    fn set_concurrency_limit(&self, key: &str, capacity: usize) {
        if let Some(limiter) = self.concurrency.get(key) {
            limiter.set_capacity(capacity);
        }
    }
}

/// Shared governor reference.
pub type SharedTokenBucketRegistry = Arc<TokenBucketRegistry>;
