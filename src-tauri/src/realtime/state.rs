use std::collections::BTreeMap;

use serde_json::Value;
use tokio::task::JoinHandle;

use crate::{config::ApiCredentials, tick_collector::TickEvent};

mod private;
mod status;

pub(super) use self::private::{
    has_any_private_subscriptions_for_mode, has_private_business_subscriptions_for_mode,
    has_private_subscriptions_for_mode, has_public_subscriptions, private_refs_mut, PrivateChannel,
};

#[derive(Debug)]
pub(super) struct RealtimeState {
    pub(super) ticker_refs: BTreeMap<String, usize>,
    pub(super) trade_refs: BTreeMap<String, usize>,
    pub(super) orderbook_refs: BTreeMap<String, usize>,
    pub(super) candle_refs: BTreeMap<String, usize>,
    pub(super) private_account_refs: BTreeMap<String, usize>,
    pub(super) private_order_refs: BTreeMap<String, usize>,
    pub(super) private_algo_order_refs: BTreeMap<String, usize>,
    pub(super) private_fill_refs: BTreeMap<String, usize>,
    pub(super) private_position_refs: BTreeMap<String, usize>,
    pub(super) private_generations: BTreeMap<String, u64>,
    pub(super) private_business_generations: BTreeMap<String, u64>,
    pub(super) private_connected: BTreeMap<String, bool>,
    pub(super) private_business_connected: BTreeMap<String, bool>,
    pub(super) private_last_error: BTreeMap<String, String>,
    pub(super) private_business_last_error: BTreeMap<String, String>,
    pub(super) private_last_connected_at: BTreeMap<String, i64>,
    pub(super) private_business_last_connected_at: BTreeMap<String, i64>,
    pub(super) private_last_message_at: BTreeMap<String, i64>,
    pub(super) private_business_last_message_at: BTreeMap<String, i64>,
    pub(super) private_reconnect_count: BTreeMap<String, u64>,
    pub(super) private_business_reconnect_count: BTreeMap<String, u64>,
    pub(super) private_credentials: BTreeMap<String, ApiCredentials>,
    pub(super) private_account_cache: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) private_account_summary_cache: BTreeMap<String, Value>,
    pub(super) private_order_cache: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) private_algo_order_cache: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) private_fill_cache: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) private_position_cache: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) generation: u64,
    pub(super) candle_generation: u64,
    pub(super) connected: bool,
    pub(super) candle_connected: bool,
    pub(super) last_error: Option<String>,
    pub(super) candle_last_error: Option<String>,
    pub(super) last_connected_at: Option<i64>,
    pub(super) candle_last_connected_at: Option<i64>,
    pub(super) last_message_at: Option<i64>,
    pub(super) candle_last_message_at: Option<i64>,
    pub(super) reconnect_count: u64,
    pub(super) candle_reconnect_count: u64,
    pub(super) task: Option<JoinHandle<()>>,
    pub(super) candle_task: Option<JoinHandle<()>>,
    pub(super) private_tasks: BTreeMap<String, JoinHandle<()>>,
    pub(super) private_business_tasks: BTreeMap<String, JoinHandle<()>>,
    pub(super) tick_tx: Option<tokio::sync::mpsc::UnboundedSender<TickEvent>>,
    pub(super) proxy_url: String,
}

impl RealtimeState {
    pub(super) fn new(proxy_url: String) -> Self {
        Self {
            ticker_refs: BTreeMap::new(),
            trade_refs: BTreeMap::new(),
            orderbook_refs: BTreeMap::new(),
            candle_refs: BTreeMap::new(),
            private_account_refs: BTreeMap::new(),
            private_order_refs: BTreeMap::new(),
            private_algo_order_refs: BTreeMap::new(),
            private_fill_refs: BTreeMap::new(),
            private_position_refs: BTreeMap::new(),
            private_generations: BTreeMap::new(),
            private_business_generations: BTreeMap::new(),
            private_connected: BTreeMap::new(),
            private_business_connected: BTreeMap::new(),
            private_last_error: BTreeMap::new(),
            private_business_last_error: BTreeMap::new(),
            private_last_connected_at: BTreeMap::new(),
            private_business_last_connected_at: BTreeMap::new(),
            private_last_message_at: BTreeMap::new(),
            private_business_last_message_at: BTreeMap::new(),
            private_reconnect_count: BTreeMap::new(),
            private_business_reconnect_count: BTreeMap::new(),
            private_credentials: BTreeMap::new(),
            private_account_cache: BTreeMap::new(),
            private_account_summary_cache: BTreeMap::new(),
            private_order_cache: BTreeMap::new(),
            private_algo_order_cache: BTreeMap::new(),
            private_fill_cache: BTreeMap::new(),
            private_position_cache: BTreeMap::new(),
            generation: 0,
            candle_generation: 0,
            connected: false,
            candle_connected: false,
            last_error: None,
            candle_last_error: None,
            last_connected_at: None,
            candle_last_connected_at: None,
            last_message_at: None,
            candle_last_message_at: None,
            reconnect_count: 0,
            candle_reconnect_count: 0,
            task: None,
            candle_task: None,
            private_tasks: BTreeMap::new(),
            private_business_tasks: BTreeMap::new(),
            tick_tx: None,
            proxy_url,
        }
    }
}
