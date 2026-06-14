use super::super::LiveStrategyRuntime;

impl LiveStrategyRuntime {
    pub(in crate::live_strategy) async fn set_error(&self, run_id: &str, message: String) {
        let mut status = self.inner.status.write().await;
        if status.run_id == run_id {
            status.error_message = message;
        }
    }

    pub(in crate::live_strategy) async fn mark_stopped(&self, run_id: &str) {
        let mut status = self.inner.status.write().await;
        if status.run_id == run_id {
            status.status = "stopped".to_string();
            status.error_message.clear();
        }
    }
}
