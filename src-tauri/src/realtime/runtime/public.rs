use super::super::{normalize::now_ms, RealtimeManager};

impl RealtimeManager {
    pub(in crate::realtime) async fn mark_connected(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return;
        }
        let now = now_ms();
        state.connected = true;
        state.last_connected_at = Some(now);
        state.last_message_at = Some(now);
        state.last_error = None;
    }

    pub(in crate::realtime) async fn mark_message(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.last_message_at = Some(now_ms());
        }
    }

    pub(in crate::realtime) async fn set_last_error(&self, generation: u64, error: Option<String>) {
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.last_error = error;
        }
    }

    pub(in crate::realtime) async fn mark_disconnected(
        &self,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return;
        }
        state.connected = false;
        state.last_error = error;
        state.reconnect_count = state.reconnect_count.saturating_add(1);
    }

    pub(in crate::realtime) async fn mark_worker_finished(&self, generation: u64) {
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return;
        }
        state.connected = false;
        state.task = None;
    }
}
