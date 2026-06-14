use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::super::{
    normalize::{parse_candle_key, parse_orderbook_key},
    RealtimeManager, PRIVATE_INST_TYPES,
};

impl RealtimeManager {
    pub(in crate::realtime) async fn active_public_subscriptions(&self) -> Vec<Value> {
        let state = self.state.lock().await;
        let mut args = Vec::with_capacity(
            state.ticker_refs.len() + state.trade_refs.len() + state.orderbook_refs.len(),
        );
        args.extend(
            state
                .ticker_refs
                .keys()
                .map(|inst_id| json!({"channel": "tickers", "instId": inst_id})),
        );
        args.extend(
            state
                .trade_refs
                .keys()
                .map(|inst_id| json!({"channel": "trades", "instId": inst_id})),
        );
        args.extend(
            state
                .orderbook_refs
                .keys()
                .filter_map(|key| parse_orderbook_key(key))
                .map(|(inst_id, channel)| json!({"channel": channel, "instId": inst_id})),
        );
        args
    }

    pub(in crate::realtime) async fn active_candles(&self) -> Vec<(String, String)> {
        let state = self.state.lock().await;
        state
            .candle_refs
            .keys()
            .filter_map(|key| parse_candle_key(key))
            .collect()
    }

    pub(in crate::realtime) async fn active_private_modes(&self) -> Vec<String> {
        let state = self.state.lock().await;
        let mut modes = BTreeSet::new();
        modes.extend(state.private_account_refs.keys().cloned());
        modes.extend(state.private_order_refs.keys().cloned());
        modes.extend(state.private_fill_refs.keys().cloned());
        modes.extend(state.private_position_refs.keys().cloned());
        modes.into_iter().collect()
    }

    pub(in crate::realtime) async fn active_private_business_modes(&self) -> Vec<String> {
        let state = self.state.lock().await;
        state.private_algo_order_refs.keys().cloned().collect()
    }

    pub(in crate::realtime) async fn active_private_args(&self, mode: &str) -> Vec<Value> {
        let state = self.state.lock().await;
        let mut args = Vec::new();
        if state.private_account_refs.contains_key(mode) {
            args.push(json!({"channel": "account"}));
        }
        if state.private_order_refs.contains_key(mode) {
            args.push(json!({"channel": "orders", "instType": "ANY"}));
        }
        if state.private_fill_refs.contains_key(mode) {
            for inst_type in PRIVATE_INST_TYPES {
                args.push(json!({"channel": "fills", "instType": inst_type}));
            }
        }
        if state.private_position_refs.contains_key(mode) {
            args.push(json!({
                "channel": "positions",
                "instType": "ANY",
                "extraParams": json!({"updateInterval": "2000"}).to_string()
            }));
        }
        args
    }

    pub(in crate::realtime) async fn active_private_business_args(&self, mode: &str) -> Vec<Value> {
        let state = self.state.lock().await;
        let mut args = Vec::new();
        if state.private_algo_order_refs.contains_key(mode) {
            args.push(json!({"channel": "orders-algo", "instType": "ANY"}));
        }
        args
    }
}
