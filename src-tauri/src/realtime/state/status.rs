use serde::Serialize;
use serde_json::{json, Value};

use crate::okx_network::{
    effective_proxy_label, OKX_BUSINESS_WS_URL, OKX_BUSINESS_WS_URL_SIMULATED,
    OKX_PRIVATE_WS_URL_LIVE, OKX_PRIVATE_WS_URL_SIMULATED, OKX_PUBLIC_WS_URL,
};

use super::super::{
    ACCOUNT_EVENT, ALERT_EVENT, ALGO_ORDER_EVENT, CANDLE_EVENT, FILL_EVENT, ORDERBOOK_EVENT,
    ORDER_EVENT, POSITION_EVENT, TICKER_EVENT, TRADE_EVENT,
};
use super::RealtimeState;

impl RealtimeState {
    pub(in crate::realtime) fn status_payload(&self) -> Value {
        let mut payload = serde_json::Map::new();
        payload.insert("connected".to_string(), Value::Bool(self.connected));
        payload.insert(
            "candle_connected".to_string(),
            Value::Bool(self.candle_connected),
        );
        payload.insert(
            "private_connected".to_string(),
            json_value(&self.private_connected),
        );
        payload.insert(
            "private_business_connected".to_string(),
            json_value(&self.private_business_connected),
        );
        payload.insert(
            "subscribed_tickers".to_string(),
            json_value(self.ticker_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_trades".to_string(),
            json_value(self.trade_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_orderbooks".to_string(),
            json_value(self.orderbook_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_candles".to_string(),
            json_value(self.candle_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_accounts".to_string(),
            json_value(
                self.private_account_refs
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        );
        payload.insert(
            "subscribed_orders".to_string(),
            json_value(self.private_order_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_algo_orders".to_string(),
            json_value(
                self.private_algo_order_refs
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        );
        payload.insert(
            "subscribed_fills".to_string(),
            json_value(self.private_fill_refs.keys().cloned().collect::<Vec<_>>()),
        );
        payload.insert(
            "subscribed_positions".to_string(),
            json_value(
                self.private_position_refs
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        );
        payload.insert(
            "ticker_ref_counts".to_string(),
            json_value(&self.ticker_refs),
        );
        payload.insert("trade_ref_counts".to_string(), json_value(&self.trade_refs));
        payload.insert(
            "orderbook_ref_counts".to_string(),
            json_value(&self.orderbook_refs),
        );
        payload.insert(
            "candle_ref_counts".to_string(),
            json_value(&self.candle_refs),
        );
        payload.insert(
            "account_ref_counts".to_string(),
            json_value(&self.private_account_refs),
        );
        payload.insert(
            "private_order_ref_counts".to_string(),
            json_value(&self.private_order_refs),
        );
        payload.insert(
            "private_algo_order_ref_counts".to_string(),
            json_value(&self.private_algo_order_refs),
        );
        payload.insert(
            "private_fill_ref_counts".to_string(),
            json_value(&self.private_fill_refs),
        );
        payload.insert(
            "private_position_ref_counts".to_string(),
            json_value(&self.private_position_refs),
        );
        payload.insert(
            "active_subscriptions".to_string(),
            json_value(
                self.ticker_refs.len()
                    + self.trade_refs.len()
                    + self.orderbook_refs.len()
                    + self.private_account_refs.len()
                    + self.private_order_refs.len()
                    + self.private_algo_order_refs.len()
                    + self.private_fill_refs.len()
                    + self.private_position_refs.len(),
            ),
        );
        payload.insert(
            "active_ticker_subscriptions".to_string(),
            json_value(self.ticker_refs.len()),
        );
        payload.insert(
            "active_trade_subscriptions".to_string(),
            json_value(self.trade_refs.len()),
        );
        payload.insert(
            "active_orderbook_subscriptions".to_string(),
            json_value(self.orderbook_refs.len()),
        );
        payload.insert(
            "active_candle_subscriptions".to_string(),
            json_value(self.candle_refs.len()),
        );
        payload.insert(
            "active_account_subscriptions".to_string(),
            json_value(self.private_account_refs.len()),
        );
        payload.insert(
            "active_private_order_subscriptions".to_string(),
            json_value(self.private_order_refs.len()),
        );
        payload.insert(
            "active_private_algo_order_subscriptions".to_string(),
            json_value(self.private_algo_order_refs.len()),
        );
        payload.insert(
            "active_private_fill_subscriptions".to_string(),
            json_value(self.private_fill_refs.len()),
        );
        payload.insert(
            "active_private_position_subscriptions".to_string(),
            json_value(self.private_position_refs.len()),
        );
        payload.insert("generation".to_string(), json_value(self.generation));
        payload.insert(
            "candle_generation".to_string(),
            json_value(self.candle_generation),
        );
        payload.insert(
            "private_generations".to_string(),
            json_value(&self.private_generations),
        );
        payload.insert(
            "private_business_generations".to_string(),
            json_value(&self.private_business_generations),
        );
        payload.insert("last_error".to_string(), json_value(&self.last_error));
        payload.insert(
            "candle_last_error".to_string(),
            json_value(&self.candle_last_error),
        );
        payload.insert(
            "private_last_error".to_string(),
            json_value(&self.private_last_error),
        );
        payload.insert(
            "private_business_last_error".to_string(),
            json_value(&self.private_business_last_error),
        );
        payload.insert(
            "last_connected_at".to_string(),
            json_value(self.last_connected_at),
        );
        payload.insert(
            "candle_last_connected_at".to_string(),
            json_value(self.candle_last_connected_at),
        );
        payload.insert(
            "private_last_connected_at".to_string(),
            json_value(&self.private_last_connected_at),
        );
        payload.insert(
            "private_business_last_connected_at".to_string(),
            json_value(&self.private_business_last_connected_at),
        );
        payload.insert(
            "last_message_at".to_string(),
            json_value(self.last_message_at),
        );
        payload.insert(
            "candle_last_message_at".to_string(),
            json_value(self.candle_last_message_at),
        );
        payload.insert(
            "private_last_message_at".to_string(),
            json_value(&self.private_last_message_at),
        );
        payload.insert(
            "private_business_last_message_at".to_string(),
            json_value(&self.private_business_last_message_at),
        );
        payload.insert(
            "reconnect_count".to_string(),
            json_value(self.reconnect_count),
        );
        payload.insert(
            "candle_reconnect_count".to_string(),
            json_value(self.candle_reconnect_count),
        );
        payload.insert(
            "private_reconnect_count".to_string(),
            json_value(&self.private_reconnect_count),
        );
        payload.insert(
            "private_business_reconnect_count".to_string(),
            json_value(&self.private_business_reconnect_count),
        );
        let proxy_label = effective_proxy_label(&self.proxy_url);
        payload.insert(
            "proxy_enabled".to_string(),
            Value::Bool(!proxy_label.is_empty()),
        );
        payload.insert("proxy_url".to_string(), Value::String(proxy_label));
        payload.insert(
            "public_url".to_string(),
            Value::String(OKX_PUBLIC_WS_URL.to_string()),
        );
        payload.insert(
            "business_url".to_string(),
            Value::String(OKX_BUSINESS_WS_URL.to_string()),
        );
        payload.insert(
            "business_urls".to_string(),
            json!({
                "live": OKX_BUSINESS_WS_URL,
                "simulated": OKX_BUSINESS_WS_URL_SIMULATED
            }),
        );
        payload.insert(
            "private_urls".to_string(),
            json!({
                "live": OKX_PRIVATE_WS_URL_LIVE,
                "simulated": OKX_PRIVATE_WS_URL_SIMULATED
            }),
        );
        payload.insert(
            "events".to_string(),
            json!({
                "ticker": TICKER_EVENT,
                "candle": CANDLE_EVENT,
                "trade": TRADE_EVENT,
                "orderbook": ORDERBOOK_EVENT,
                "alert": ALERT_EVENT,
                "account": ACCOUNT_EVENT,
                "order": ORDER_EVENT,
                "algo_order": ALGO_ORDER_EVENT,
                "fill": FILL_EVENT,
                "position": POSITION_EVENT
            }),
        );
        Value::Object(payload)
    }
}

fn json_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}
