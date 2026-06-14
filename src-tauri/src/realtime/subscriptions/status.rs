use serde_json::Value;

use super::super::RealtimeManager;

impl RealtimeManager {
    pub async fn status(&self) -> Value {
        let state = self.state.lock().await;
        state.status_payload()
    }
}
