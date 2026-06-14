use std::{collections::BTreeMap, sync::Arc, time::Duration};

use hmac::Hmac;
use sha2::Sha256;
use sqlx::SqlitePool;
use tauri::AppHandle;
use tokio::sync::{broadcast, Mutex};

use crate::{
    live_strategy::LiveStrategyRuntime,
    okx_outbound::{OKXOutboundTimelineStore, OKXRateRuleRegistry},
    token_bucket::SharedTokenBucketRegistry,
};

pub const TICKER_EVENT: &str = "okxq-market-ticker";
pub const CANDLE_EVENT: &str = "okxq-market-candle";
pub const TRADE_EVENT: &str = "okxq-market-trade";
pub const ORDERBOOK_EVENT: &str = "okxq-market-orderbook";
pub const ALERT_EVENT: &str = "okxq-market-alert";
pub const ACCOUNT_EVENT: &str = "okxq-private-account";
pub const ORDER_EVENT: &str = "okxq-private-order";
pub const ALGO_ORDER_EVENT: &str = "okxq-private-algo-order";
pub const FILL_EVENT: &str = "okxq-private-fill";
pub const POSITION_EVENT: &str = "okxq-private-position";

const DEFAULT_ORDERBOOK_CHANNEL: &str = "books5";
const PRIVATE_INST_TYPES: [&str; 4] = ["SPOT", "SWAP", "FUTURES", "MARGIN"];
const WS_IDLE_TIMEOUT: Duration = Duration::from_secs(25);
const WS_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const WS_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(15);
const PRIVATE_WS_CACHE_MAX_AGE_MS: i64 = 120_000;
type HmacSha256 = Hmac<Sha256>;

mod emissions;
mod messages;
mod normalize;
mod runtime;
mod state;
mod subscriptions;
mod transport;

use self::state::RealtimeState;

#[derive(Clone)]
pub struct RealtimeManager {
    app: AppHandle,
    db: SqlitePool,
    state: Arc<Mutex<RealtimeState>>,
    candle_event_tx: broadcast::Sender<RealtimeCandleEvent>,
    live_strategy: LiveStrategyRuntime,
    outbound_timeline: Arc<OKXOutboundTimelineStore>,
    rate_rules: Arc<OKXRateRuleRegistry>,
    outbound_governor: SharedTokenBucketRegistry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RealtimeCandleEvent {
    pub(crate) inst_id: String,
    pub(crate) timeframe: String,
    pub(crate) timestamp: i64,
}

impl RealtimeManager {
    pub fn new(
        app: AppHandle,
        db: SqlitePool,
        proxy_url: String,
        live_strategy: LiveStrategyRuntime,
        outbound_timeline: Arc<OKXOutboundTimelineStore>,
        rate_rules: Arc<OKXRateRuleRegistry>,
        outbound_governor: SharedTokenBucketRegistry,
    ) -> Self {
        let (candle_event_tx, _) = broadcast::channel(1024);
        Self {
            app,
            db,
            state: Arc::new(Mutex::new(RealtimeState::new(proxy_url))),
            candle_event_tx,
            live_strategy,
            outbound_timeline,
            rate_rules,
            outbound_governor,
        }
    }

    pub(crate) fn subscribe_confirmed_candles(&self) -> broadcast::Receiver<RealtimeCandleEvent> {
        self.candle_event_tx.subscribe()
    }

    pub(crate) async fn latest_private_account_balance_items(
        &self,
        mode: &str,
    ) -> Option<Vec<serde_json::Value>> {
        let mode = private_cache_mode(mode);
        let state = self.state.lock().await;
        if !private_regular_cache_is_fresh(&state, &mode) {
            return None;
        }
        let summary = state.private_account_summary_cache.get(&mode)?;
        let mut raw = summary.get("raw")?.clone();
        if let Some(received_at_ms) = summary
            .get("_received_at_ms")
            .and_then(serde_json::Value::as_i64)
        {
            if let Some(object) = raw.as_object_mut() {
                object.insert(
                    "_okxq_received_at_ms".to_string(),
                    serde_json::json!(received_at_ms),
                );
            }
        }
        raw.is_object().then_some(vec![raw])
    }

    pub(crate) async fn latest_private_position_raw_items(
        &self,
        mode: &str,
        inst_type: &str,
    ) -> Option<Vec<serde_json::Value>> {
        let mode = private_cache_mode(mode);
        let state = self.state.lock().await;
        if !private_regular_cache_is_fresh(&state, &mode) {
            return None;
        }
        let cache = state.private_position_cache.get(&mode)?;
        non_empty_raw_private_cache_items(cache, inst_type)
    }

    pub(crate) async fn latest_private_order_raw_items(
        &self,
        mode: &str,
        inst_type: &str,
    ) -> Option<Vec<serde_json::Value>> {
        let mode = private_cache_mode(mode);
        let state = self.state.lock().await;
        if !private_regular_cache_is_fresh(&state, &mode) {
            return None;
        }
        let cache = state.private_order_cache.get(&mode)?;
        non_empty_raw_private_cache_items(cache, inst_type)
    }

    pub(crate) async fn latest_private_algo_order_raw_items(
        &self,
        mode: &str,
        inst_type: &str,
    ) -> Option<Vec<serde_json::Value>> {
        let mode = private_cache_mode(mode);
        let state = self.state.lock().await;
        if !private_business_cache_is_fresh(&state, &mode) {
            return None;
        }
        let cache = state.private_algo_order_cache.get(&mode)?;
        non_empty_raw_private_cache_items(cache, inst_type)
    }

    pub(crate) async fn latest_private_fill_raw_items(
        &self,
        mode: &str,
        inst_type: &str,
    ) -> Option<Vec<serde_json::Value>> {
        let mode = private_cache_mode(mode);
        let state = self.state.lock().await;
        if !private_regular_cache_is_fresh(&state, &mode) {
            return None;
        }
        let cache = state.private_fill_cache.get(&mode)?;
        non_empty_raw_private_cache_items(cache, inst_type)
    }
}

fn private_cache_mode(mode: &str) -> String {
    if mode.trim().eq_ignore_ascii_case("live") {
        "live".to_string()
    } else {
        "simulated".to_string()
    }
}

fn private_regular_cache_is_fresh(state: &RealtimeState, mode: &str) -> bool {
    if state.private_connected.get(mode).copied() != Some(true) {
        return false;
    }
    let Some(last_message_at) = state.private_last_message_at.get(mode).copied() else {
        return false;
    };
    let now = chrono::Utc::now().timestamp_millis();
    now.saturating_sub(last_message_at) <= PRIVATE_WS_CACHE_MAX_AGE_MS
}

fn private_business_cache_is_fresh(state: &RealtimeState, mode: &str) -> bool {
    if state.private_business_connected.get(mode).copied() != Some(true) {
        return false;
    }
    let Some(last_message_at) = state.private_business_last_message_at.get(mode).copied() else {
        return false;
    };
    let now = chrono::Utc::now().timestamp_millis();
    now.saturating_sub(last_message_at) <= PRIVATE_WS_CACHE_MAX_AGE_MS
}

fn raw_private_cache_items(
    cache: &BTreeMap<String, serde_json::Value>,
    inst_type: &str,
) -> Vec<serde_json::Value> {
    let normalized_inst_type = inst_type.trim().to_ascii_uppercase();
    cache
        .values()
        .filter(|item| {
            normalized_inst_type.is_empty()
                || item
                    .get("inst_type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case(&normalized_inst_type))
        })
        .filter_map(|item| item.get("raw").cloned())
        .filter(serde_json::Value::is_object)
        .collect()
}

fn non_empty_raw_private_cache_items(
    cache: &BTreeMap<String, serde_json::Value>,
    inst_type: &str,
) -> Option<Vec<serde_json::Value>> {
    let rows = raw_private_cache_items(cache, inst_type);
    (!rows.is_empty()).then_some(rows)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        non_empty_raw_private_cache_items, private_business_cache_is_fresh,
        private_regular_cache_is_fresh, raw_private_cache_items, RealtimeState,
        PRIVATE_WS_CACHE_MAX_AGE_MS,
    };

    #[test]
    fn private_regular_cache_requires_connected_recent_message() {
        let mut state = RealtimeState::new(String::new());
        let mode = "simulated".to_string();
        assert!(!private_regular_cache_is_fresh(&state, &mode));

        state.private_connected.insert(mode.clone(), true);
        state.private_last_message_at.insert(
            mode.clone(),
            chrono::Utc::now().timestamp_millis() - PRIVATE_WS_CACHE_MAX_AGE_MS - 1,
        );
        assert!(!private_regular_cache_is_fresh(&state, &mode));

        state
            .private_last_message_at
            .insert(mode.clone(), chrono::Utc::now().timestamp_millis() - 1_000);
        assert!(private_regular_cache_is_fresh(&state, &mode));

        state.private_connected.insert(mode.clone(), false);
        assert!(!private_regular_cache_is_fresh(&state, &mode));
    }

    #[test]
    fn private_business_cache_requires_business_connection_recency() {
        let mut state = RealtimeState::new(String::new());
        let mode = "simulated".to_string();
        state.private_connected.insert(mode.clone(), true);
        state
            .private_last_message_at
            .insert(mode.clone(), chrono::Utc::now().timestamp_millis());
        assert!(!private_business_cache_is_fresh(&state, &mode));

        state.private_business_connected.insert(mode.clone(), true);
        state.private_business_last_message_at.insert(
            mode.clone(),
            chrono::Utc::now().timestamp_millis() - PRIVATE_WS_CACHE_MAX_AGE_MS - 1,
        );
        assert!(!private_business_cache_is_fresh(&state, &mode));

        state
            .private_business_last_message_at
            .insert(mode.clone(), chrono::Utc::now().timestamp_millis() - 1_000);
        assert!(private_business_cache_is_fresh(&state, &mode));
    }

    #[test]
    fn raw_private_cache_items_filter_inst_type_and_return_raw_payloads() {
        let mut cache = std::collections::BTreeMap::new();
        cache.insert(
            "swap".to_string(),
            json!({
                "inst_type": "SWAP",
                "raw": {"instId": "BTC-USDT-SWAP", "instType": "SWAP", "ordId": "swap-order"}
            }),
        );
        cache.insert(
            "spot".to_string(),
            json!({
                "inst_type": "SPOT",
                "raw": {"instId": "BTC-USDT", "instType": "SPOT", "ordId": "spot-order"}
            }),
        );

        let rows = raw_private_cache_items(&cache, "swap");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["ordId"], "swap-order");
        assert!(non_empty_raw_private_cache_items(&cache, "futures").is_none());
    }
}
