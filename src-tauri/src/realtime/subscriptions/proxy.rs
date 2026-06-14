use super::super::RealtimeManager;

impl RealtimeManager {
    pub async fn set_proxy_url(&self, proxy_url: String) {
        let should_restart = {
            let mut state = self.state.lock().await;
            if state.proxy_url == proxy_url {
                false
            } else {
                state.proxy_url = proxy_url;
                true
            }
        };
        if !should_restart {
            return;
        }
        let proxy_label = self.proxy_label().await;
        tracing::info!(
            proxy = proxy_label.as_str(),
            "OKX realtime proxy configuration changed; restarting active websocket workers"
        );
        self.restart_worker().await;
        self.restart_candle_worker().await;
        let private_modes = self.active_private_modes().await;
        for mode in private_modes {
            self.restart_private_worker(&mode).await;
        }
        let private_business_modes = self.active_private_business_modes().await;
        for mode in private_business_modes {
            self.restart_private_business_worker(&mode).await;
        }
    }
}
