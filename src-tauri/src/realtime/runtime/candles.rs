use super::super::{normalize::now_ms, RealtimeManager};

impl RealtimeManager {
    pub(in crate::realtime) async fn mark_candles_connected(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.candle_generation != generation {
            return;
        }
        let now = now_ms();
        state.candle_connected = true;
        state.candle_last_connected_at = Some(now);
        state.candle_last_message_at = Some(now);
        state.candle_last_error = None;
    }

    pub(in crate::realtime) async fn mark_candle_message(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.candle_generation == generation {
            state.candle_last_message_at = Some(now_ms());
        }
    }

    pub(in crate::realtime) async fn set_candle_last_error(
        &self,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.candle_generation == generation {
            state.candle_last_error = error;
        }
    }

    pub(in crate::realtime) async fn mark_candles_disconnected(
        &self,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.candle_generation != generation {
            return;
        }
        state.candle_connected = false;
        state.candle_last_error = error;
        state.candle_reconnect_count = state.candle_reconnect_count.saturating_add(1);
    }

    pub(in crate::realtime) async fn mark_candle_worker_finished(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.candle_generation != generation {
            return;
        }
        state.candle_connected = false;
        state.candle_task = None;
    }
}
