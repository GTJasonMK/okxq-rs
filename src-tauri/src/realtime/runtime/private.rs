use super::super::{normalize::now_ms, RealtimeManager};

impl RealtimeManager {
    pub(in crate::realtime) async fn mark_private_connected(&self, mode: &str, generation: u64) {
        let mut state = self.state.lock().await;
        if state.private_generations.get(mode).copied() != Some(generation) {
            return;
        }
        let now = now_ms();
        state.private_connected.insert(mode.to_string(), true);
        state
            .private_last_connected_at
            .insert(mode.to_string(), now);
        state.private_last_message_at.insert(mode.to_string(), now);
        state.private_last_error.remove(mode);
    }

    pub(in crate::realtime) async fn mark_private_message(&self, mode: &str, generation: u64) {
        let mut state = self.state.lock().await;
        if state.private_generations.get(mode).copied() == Some(generation) {
            state
                .private_last_message_at
                .insert(mode.to_string(), now_ms());
        }
    }

    pub(in crate::realtime) async fn set_private_last_error(
        &self,
        mode: &str,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.private_generations.get(mode).copied() != Some(generation) {
            return;
        }
        if let Some(error) = error {
            state.private_last_error.insert(mode.to_string(), error);
        } else {
            state.private_last_error.remove(mode);
        }
    }

    pub(in crate::realtime) async fn mark_private_disconnected(
        &self,
        mode: &str,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.private_generations.get(mode).copied() != Some(generation) {
            return;
        }
        state.private_connected.insert(mode.to_string(), false);
        if let Some(error) = error {
            state.private_last_error.insert(mode.to_string(), error);
        }
        *state
            .private_reconnect_count
            .entry(mode.to_string())
            .or_insert(0) += 1;
    }

    pub(in crate::realtime) async fn mark_private_worker_finished(
        &self,
        mode: &str,
        generation: u64,
    ) {
        let mut state = self.state.lock().await;
        if state.private_generations.get(mode).copied() != Some(generation) {
            return;
        }
        state.private_connected.insert(mode.to_string(), false);
        state.private_tasks.remove(mode);
    }

    pub(in crate::realtime) async fn mark_private_business_connected(
        &self,
        mode: &str,
        generation: u64,
    ) {
        let mut state = self.state.lock().await;
        if state.private_business_generations.get(mode).copied() != Some(generation) {
            return;
        }
        let now = now_ms();
        state
            .private_business_connected
            .insert(mode.to_string(), true);
        state
            .private_business_last_connected_at
            .insert(mode.to_string(), now);
        state
            .private_business_last_message_at
            .insert(mode.to_string(), now);
        state.private_business_last_error.remove(mode);
    }

    pub(in crate::realtime) async fn mark_private_business_message(
        &self,
        mode: &str,
        generation: u64,
    ) {
        let mut state = self.state.lock().await;
        if state.private_business_generations.get(mode).copied() == Some(generation) {
            state
                .private_business_last_message_at
                .insert(mode.to_string(), now_ms());
        }
    }

    pub(in crate::realtime) async fn set_private_business_last_error(
        &self,
        mode: &str,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.private_business_generations.get(mode).copied() != Some(generation) {
            return;
        }
        if let Some(error) = error {
            state
                .private_business_last_error
                .insert(mode.to_string(), error);
        } else {
            state.private_business_last_error.remove(mode);
        }
    }

    pub(in crate::realtime) async fn mark_private_business_disconnected(
        &self,
        mode: &str,
        generation: u64,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.private_business_generations.get(mode).copied() != Some(generation) {
            return;
        }
        state
            .private_business_connected
            .insert(mode.to_string(), false);
        if let Some(error) = error {
            state
                .private_business_last_error
                .insert(mode.to_string(), error);
        }
        *state
            .private_business_reconnect_count
            .entry(mode.to_string())
            .or_insert(0) += 1;
    }

    pub(in crate::realtime) async fn mark_private_business_worker_finished(
        &self,
        mode: &str,
        generation: u64,
    ) {
        let mut state = self.state.lock().await;
        if state.private_business_generations.get(mode).copied() != Some(generation) {
            return;
        }
        state
            .private_business_connected
            .insert(mode.to_string(), false);
        state.private_business_tasks.remove(mode);
    }
}
