use serde_json::Value;

use crate::{error::AppResult, tick_collector::TickEvent};

use super::{
    super::{
        normalize::{normalize_orderbook_channel, orderbook_key},
        RealtimeManager,
    },
    refs::{decrement_ref, normalize_inst_ids},
};

impl RealtimeManager {
    /// 注入秒级数据采集器的发送端，使 WebSocket 数据同时写入数据库。
    pub async fn set_tick_collector_tx(&self, tx: tokio::sync::mpsc::UnboundedSender<TickEvent>) {
        let mut state = self.state.lock().await;
        state.tick_tx = Some(tx);
    }

    /// 清理秒级采集器发送端，避免停止采集后继续向已关闭通道推送数据。
    pub async fn clear_tick_collector_tx(&self) {
        let mut state = self.state.lock().await;
        state.tick_tx = None;
    }

    /// 给秒级采集器批量订阅成交和盘口，只重启一次公共 WebSocket worker。
    pub async fn subscribe_collection_feeds(
        &self,
        inst_ids: &[String],
        channel: &str,
    ) -> AppResult<Value> {
        let channel = normalize_orderbook_channel(channel)?;
        let inst_ids = normalize_inst_ids(inst_ids)?;
        {
            let mut state = self.state.lock().await;
            for inst_id in &inst_ids {
                *state.trade_refs.entry(inst_id.clone()).or_insert(0) += 1;
                *state
                    .orderbook_refs
                    .entry(orderbook_key(inst_id, &channel))
                    .or_insert(0) += 1;
            }
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }

    /// 停止秒级采集时释放批量订阅引用，不影响用户手动打开的其它引用。
    pub async fn unsubscribe_collection_feeds(
        &self,
        inst_ids: &[String],
        channel: &str,
    ) -> AppResult<Value> {
        let channel = normalize_orderbook_channel(channel)?;
        let inst_ids = normalize_inst_ids(inst_ids)?;
        {
            let mut state = self.state.lock().await;
            for inst_id in &inst_ids {
                decrement_ref(&mut state.trade_refs, inst_id);
                decrement_ref(&mut state.orderbook_refs, &orderbook_key(inst_id, &channel));
            }
        }
        self.restart_worker().await;
        Ok(self.status().await)
    }
}
