use serde_json::Value;

use crate::error::AppResult;

use super::{
    super::{
        normalize::{
            candle_key, normalize_inst_id, normalize_orderbook_channel, normalize_timeframe,
            orderbook_key,
        },
        RealtimeManager,
    },
    refs::decrement_ref,
};

impl RealtimeManager {
    pub async fn subscribe_ticker(&self, inst_id: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        {
            let mut state = self.state.lock().await;
            *state.ticker_refs.entry(inst_id).or_insert(0) += 1;
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn unsubscribe_ticker(&self, inst_id: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        {
            let mut state = self.state.lock().await;
            decrement_ref(&mut state.ticker_refs, &inst_id);
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn subscribe_trades(&self, inst_id: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        {
            let mut state = self.state.lock().await;
            *state.trade_refs.entry(inst_id).or_insert(0) += 1;
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn unsubscribe_trades(&self, inst_id: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        {
            let mut state = self.state.lock().await;
            decrement_ref(&mut state.trade_refs, &inst_id);
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn subscribe_orderbook(&self, inst_id: &str, channel: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        let channel = normalize_orderbook_channel(channel)?;
        {
            let mut state = self.state.lock().await;
            *state
                .orderbook_refs
                .entry(orderbook_key(&inst_id, &channel))
                .or_insert(0) += 1;
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn unsubscribe_orderbook(&self, inst_id: &str, channel: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        let channel = normalize_orderbook_channel(channel)?;
        let key = orderbook_key(&inst_id, &channel);
        {
            let mut state = self.state.lock().await;
            decrement_ref(&mut state.orderbook_refs, &key);
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    pub async fn subscribe_candle(&self, inst_id: &str, timeframe: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        let timeframe = normalize_timeframe(timeframe)?;
        {
            let mut state = self.state.lock().await;
            *state
                .candle_refs
                .entry(candle_key(&inst_id, &timeframe))
                .or_insert(0) += 1;
        }
        self.restart_candle_worker().await;
        Ok(self.status().await)
    }

    pub(crate) async fn subscribe_candles(
        &self,
        subscriptions: &[(String, String)],
    ) -> AppResult<Value> {
        let subscriptions = normalize_candle_subscriptions(subscriptions)?;
        {
            let mut state = self.state.lock().await;
            for (inst_id, timeframe) in &subscriptions {
                *state
                    .candle_refs
                    .entry(candle_key(inst_id, timeframe))
                    .or_insert(0) += 1;
            }
        }
        self.restart_candle_worker().await;
        Ok(self.status().await)
    }

    pub async fn unsubscribe_candle(&self, inst_id: &str, timeframe: &str) -> AppResult<Value> {
        let inst_id = normalize_inst_id(inst_id)?;
        let timeframe = normalize_timeframe(timeframe)?;
        let key = candle_key(&inst_id, &timeframe);
        {
            let mut state = self.state.lock().await;
            decrement_ref(&mut state.candle_refs, &key);
        }
        self.restart_candle_worker().await;
        Ok(self.status().await)
    }

    pub(crate) async fn unsubscribe_candles(
        &self,
        subscriptions: &[(String, String)],
    ) -> AppResult<Value> {
        let subscriptions = normalize_candle_subscriptions(subscriptions)?;
        {
            let mut state = self.state.lock().await;
            for (inst_id, timeframe) in &subscriptions {
                decrement_ref(&mut state.candle_refs, &candle_key(inst_id, timeframe));
            }
        }
        self.restart_candle_worker().await;
        Ok(self.status().await)
    }
}

fn normalize_candle_subscriptions(
    subscriptions: &[(String, String)],
) -> AppResult<Vec<(String, String)>> {
    let mut normalized = Vec::new();
    for (inst_id, timeframe) in subscriptions {
        let inst_id = normalize_inst_id(inst_id)?;
        let timeframe = normalize_timeframe(timeframe)?;
        if !normalized.iter().any(|(known_inst_id, known_timeframe)| {
            known_inst_id == &inst_id && known_timeframe == &timeframe
        }) {
            normalized.push((inst_id, timeframe));
        }
    }
    Ok(normalized)
}
