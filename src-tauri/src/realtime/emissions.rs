use std::collections::BTreeMap;

use serde_json::{json, Value};
use tauri::Emitter;

use crate::{
    alerts::{self, TickerSnapshot},
    instrument::infer_spot_swap_inst_type,
    live_strategy::{
        persist_private_algo_order_event, persist_private_fill_event, persist_private_order_event,
    },
    tick_collector::TickEvent,
};

use super::{
    normalize::{
        normalize_private_account_detail, normalize_private_account_summary,
        normalize_private_mode, now_ms, persist_confirmed_candle,
    },
    RealtimeManager, ACCOUNT_EVENT, ALERT_EVENT, ALGO_ORDER_EVENT, CANDLE_EVENT, FILL_EVENT,
    ORDERBOOK_EVENT, ORDER_EVENT, POSITION_EVENT, TICKER_EVENT, TRADE_EVENT,
};

const PRIVATE_ORDER_CACHE_LIMIT: usize = 300;
const PRIVATE_ALGO_ORDER_CACHE_LIMIT: usize = 300;
const PRIVATE_FILL_CACHE_LIMIT: usize = 300;

impl RealtimeManager {
    pub(super) async fn emit_ticker(&self, ticker: Value) {
        if let Err(error) = self.app.emit(TICKER_EVENT, ticker.clone()) {
            tracing::warn!("emit ticker event failed: {error}");
        }

        let inst_id = ticker
            .get("inst_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let inst_type = ticker
            .get("inst_type")
            .and_then(Value::as_str)
            .unwrap_or_else(|| infer_spot_swap_inst_type(&inst_id))
            .to_string();
        let last_price = ticker.get("last").and_then(Value::as_f64).unwrap_or(0.0);
        if !last_price.is_finite() || last_price <= 0.0 {
            return;
        }
        let change_24h = ticker.get("change24h").and_then(Value::as_f64);
        let ticker_ts = ticker
            .get("ts")
            .and_then(Value::as_i64)
            .unwrap_or_else(now_ms);

        match alerts::evaluate_ticker(
            &self.db,
            TickerSnapshot {
                inst_id,
                inst_type,
                last_price,
                change_24h,
                ticker_ts,
            },
        )
        .await
        {
            Ok(triggered_alerts) => {
                for alert in triggered_alerts {
                    if let Err(error) = self.app.emit(ALERT_EVENT, alert) {
                        tracing::warn!("emit alert event failed: {error}");
                    }
                }
            }
            Err(error) => tracing::warn!("evaluate realtime price alerts failed: {error}"),
        }
    }

    pub(super) async fn emit_candle(&self, candle: Value) {
        if let Err(error) = self.app.emit(CANDLE_EVENT, candle.clone()) {
            tracing::warn!("emit candle event failed: {error}");
        }
        match persist_confirmed_candle(&self.db, &candle).await {
            Ok(()) => self.notify_confirmed_candle(&candle),
            Err(error) => tracing::warn!(error = %error, "persist realtime candle failed"),
        }
    }

    pub(super) async fn emit_trade(&self, trade: Value) {
        if let Err(error) = self.app.emit(TRADE_EVENT, trade.clone()) {
            tracing::warn!("emit trade event failed: {error}");
        }

        let state = self.state.lock().await;
        if let Some(ref tx) = state.tick_tx {
            let inst_id = trade
                .get("inst_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let price = trade.get("price").and_then(Value::as_f64).unwrap_or(0.0);
            let size = trade.get("size").and_then(Value::as_f64).unwrap_or(0.0);
            let side = trade
                .get("side")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let trade_id = trade
                .get("trade_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let ts = trade.get("ts").and_then(Value::as_i64).unwrap_or(0);
            let _ = tx.send(TickEvent::Trade {
                inst_id,
                price,
                size,
                side,
                trade_id,
                ts,
            });
        }
    }

    pub(super) async fn emit_orderbook(&self, orderbook: Value) {
        if let Err(error) = self.app.emit(ORDERBOOK_EVENT, orderbook.clone()) {
            tracing::warn!("emit orderbook event failed: {error}");
        }

        let state = self.state.lock().await;
        if let Some(ref tx) = state.tick_tx {
            let inst_id = orderbook
                .get("inst_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let ts = orderbook.get("ts").and_then(Value::as_i64).unwrap_or(0);

            let get_best = |side: &str| -> Option<f64> {
                let arr = orderbook.get(side).and_then(Value::as_array)?;
                let first = arr.first()?;
                first.get("price").and_then(Value::as_f64)
            };

            let bid = get_best("bids");
            let ask = get_best("asks");

            if let (Some(bid), Some(ask)) = (bid, ask) {
                let _ = tx.send(TickEvent::OrderBookMid {
                    inst_id,
                    bid,
                    ask,
                    ts,
                });
            }
        }
    }

    pub(super) async fn emit_account(&self, mode: &str, account: Value) {
        let mut account_summary = normalize_private_account_summary(&account, mode);
        account_summary["_received_at_ms"] = json!(now_ms());
        let snapshot = {
            let mut state = self.state.lock().await;
            state
                .private_account_summary_cache
                .insert(mode.to_string(), account_summary.clone());
            let cache = state
                .private_account_cache
                .entry(mode.to_string())
                .or_insert_with(BTreeMap::new);
            let details = account
                .get("details")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for detail in details {
                let Some(item) = normalize_private_account_detail(&detail, &account, mode) else {
                    continue;
                };
                let ccy = item
                    .get("ccy")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !ccy.is_empty() {
                    cache.insert(ccy, item);
                }
            }
            cache.clone()
        };
        let payload = json!({
            "mode": mode,
            "account": account_summary,
            "data": snapshot,
        });
        if let Err(error) = self.app.emit(ACCOUNT_EVENT, payload) {
            tracing::warn!("emit account event failed: {error}");
        }
    }

    pub(super) async fn emit_private_order(&self, order: Value) {
        self.cache_private_order(&order).await;
        match persist_private_order_event(&self.db, &order).await {
            Ok(outcome) => {
                if outcome.planned_exit_changed > 0 {
                    self.live_strategy.notify_planned_exit_worker();
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, order = %order, "persist private order event failed");
            }
        }
        if let Err(error) = self.app.emit(ORDER_EVENT, order) {
            tracing::warn!("emit order event failed: {error}");
        }
    }

    pub(super) async fn emit_private_algo_order(&self, order: Value) {
        self.cache_private_algo_order(&order).await;
        if let Err(error) = persist_private_algo_order_event(&self.db, &order).await {
            tracing::warn!(error = %error, order = %order, "persist private algo order event failed");
        }
        if let Err(error) = self.app.emit(ALGO_ORDER_EVENT, order) {
            tracing::warn!("emit algo order event failed: {error}");
        }
    }

    pub(super) async fn emit_private_fill(&self, fill: Value) {
        self.cache_private_fill(&fill).await;
        match persist_private_fill_event(&self.db, &fill).await {
            Ok(outcome) => {
                if outcome.planned_exit_changed > 0 {
                    self.live_strategy.notify_planned_exit_worker();
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, fill = %fill, "persist private fill event failed");
            }
        }
        if let Err(error) = self.app.emit(FILL_EVENT, fill) {
            tracing::warn!("emit fill event failed: {error}");
        }
    }

    pub(super) async fn emit_private_position(&self, position: Value) {
        self.cache_private_position(&position).await;
        if let Err(error) = self.app.emit(POSITION_EVENT, position) {
            tracing::warn!("emit position event failed: {error}");
        }
    }

    async fn cache_private_order(&self, order: &Value) {
        let Some(mode) = normalized_mode_from_private_event(order) else {
            return;
        };
        let Some(key) = private_order_cache_key(order) else {
            return;
        };
        let mut state = self.state.lock().await;
        let cache = state.private_order_cache.entry(mode).or_default();
        cache.insert(key, order.clone());
        prune_private_cache(
            cache,
            PRIVATE_ORDER_CACHE_LIMIT,
            private_order_cache_timestamp,
        );
    }

    async fn cache_private_algo_order(&self, order: &Value) {
        let Some(mode) = normalized_mode_from_private_event(order) else {
            return;
        };
        let Some(key) = private_algo_order_cache_key(order) else {
            return;
        };
        let mut state = self.state.lock().await;
        let cache = state.private_algo_order_cache.entry(mode).or_default();
        cache.insert(key, order.clone());
        prune_private_cache(
            cache,
            PRIVATE_ALGO_ORDER_CACHE_LIMIT,
            private_algo_order_cache_timestamp,
        );
    }

    async fn cache_private_fill(&self, fill: &Value) {
        let Some(mode) = normalized_mode_from_private_event(fill) else {
            return;
        };
        let Some(key) = value_text(fill, "trade_id").filter(|value| !value.is_empty()) else {
            return;
        };
        let mut state = self.state.lock().await;
        let cache = state.private_fill_cache.entry(mode).or_default();
        cache.insert(key, fill.clone());
        prune_private_cache(
            cache,
            PRIVATE_FILL_CACHE_LIMIT,
            private_fill_cache_timestamp,
        );
    }

    async fn cache_private_position(&self, position: &Value) {
        let Some(mode) = normalized_mode_from_private_event(position) else {
            return;
        };
        let Some(inst_id) = value_text(position, "inst_id").filter(|value| !value.is_empty())
        else {
            return;
        };
        let pos_side = value_text(position, "pos_side").unwrap_or_default();
        let key = format!(
            "{}|{}",
            inst_id.to_ascii_uppercase(),
            pos_side.to_ascii_lowercase()
        );
        let mut state = self.state.lock().await;
        let cache = state.private_position_cache.entry(mode).or_default();
        let pos = position.get("pos").and_then(Value::as_f64).unwrap_or(0.0);
        if pos.is_finite() && pos.abs() > f64::EPSILON {
            cache.insert(key, position.clone());
        } else {
            cache.remove(&key);
        }
    }

    fn notify_confirmed_candle(&self, candle: &Value) {
        if candle.get("confirm").and_then(Value::as_str) != Some("1") {
            return;
        }
        let inst_id = candle
            .get("inst_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        let timeframe = candle
            .get("timeframe")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let timestamp = candle.get("timestamp").and_then(Value::as_i64).unwrap_or(0);
        if inst_id.is_empty() || timeframe.is_empty() || timestamp <= 0 {
            return;
        }
        let _ = self.candle_event_tx.send(super::RealtimeCandleEvent {
            inst_id,
            timeframe,
            timestamp,
        });
    }
}

fn normalized_mode_from_private_event(value: &Value) -> Option<String> {
    let mode = value_text(value, "mode")?;
    normalize_private_mode(&mode).ok()
}

fn private_order_cache_key(order: &Value) -> Option<String> {
    value_text(order, "ord_id")
        .filter(|value| !value.is_empty())
        .map(|value| format!("ord:{}", value.to_ascii_uppercase()))
        .or_else(|| {
            value_text(order, "cl_ord_id")
                .filter(|value| !value.is_empty())
                .map(|value| format!("cl:{value}"))
        })
}

fn private_algo_order_cache_key(order: &Value) -> Option<String> {
    value_text(order, "algo_id")
        .filter(|value| !value.is_empty())
        .map(|value| format!("algo:{}", value.to_ascii_uppercase()))
        .or_else(|| {
            value_text(order, "algo_cl_ord_id")
                .filter(|value| !value.is_empty())
                .map(|value| format!("algo_cl:{value}"))
        })
}

fn private_order_cache_timestamp(order: &Value) -> i64 {
    order
        .get("u_time")
        .and_then(Value::as_i64)
        .or_else(|| order.get("c_time").and_then(Value::as_i64))
        .unwrap_or(0)
}

fn private_algo_order_cache_timestamp(order: &Value) -> i64 {
    order
        .get("u_time")
        .and_then(Value::as_i64)
        .or_else(|| order.get("c_time").and_then(Value::as_i64))
        .unwrap_or(0)
}

fn private_fill_cache_timestamp(fill: &Value) -> i64 {
    fill.get("ts").and_then(Value::as_i64).unwrap_or(0)
}

fn prune_private_cache(
    cache: &mut BTreeMap<String, Value>,
    limit: usize,
    timestamp: fn(&Value) -> i64,
) {
    if cache.len() <= limit {
        return;
    }
    let remove_count = cache.len() - limit;
    let mut keys = cache
        .iter()
        .map(|(key, value)| (timestamp(value), key.clone()))
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    for (_, key) in keys.into_iter().take(remove_count) {
        cache.remove(&key);
    }
}

fn value_text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => Some(item.trim().to_string()),
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}
