use crate::{config::ApiCredentials, okx_network::effective_proxy_label};

use super::super::{
    state::{has_private_subscriptions_for_mode, has_public_subscriptions},
    RealtimeManager,
};

impl RealtimeManager {
    pub(in crate::realtime) async fn proxy_url(&self) -> String {
        let state = self.state.lock().await;
        state.proxy_url.clone()
    }

    pub(in crate::realtime) async fn proxy_label(&self) -> String {
        effective_proxy_label(&self.proxy_url().await)
    }

    pub(in crate::realtime) async fn private_credentials(
        &self,
        mode: &str,
    ) -> Option<ApiCredentials> {
        let state = self.state.lock().await;
        state
            .private_credentials
            .get(mode)
            .filter(|credentials| credentials.is_valid())
            .cloned()
    }

    pub(in crate::realtime) async fn should_continue(&self, generation: u64) -> bool {
        let state = self.state.lock().await;
        state.generation == generation && has_public_subscriptions(&state)
    }

    pub(in crate::realtime) async fn should_continue_candles(&self, generation: u64) -> bool {
        let state = self.state.lock().await;
        state.candle_generation == generation && !state.candle_refs.is_empty()
    }

    pub(in crate::realtime) async fn should_continue_private(
        &self,
        mode: &str,
        generation: u64,
    ) -> bool {
        let state = self.state.lock().await;
        state.private_generations.get(mode).copied() == Some(generation)
            && has_private_subscriptions_for_mode(&state, mode)
    }

    pub(in crate::realtime) async fn should_continue_private_business(
        &self,
        mode: &str,
        generation: u64,
    ) -> bool {
        let state = self.state.lock().await;
        state.private_business_generations.get(mode).copied() == Some(generation)
            && super::super::state::has_private_business_subscriptions_for_mode(&state, mode)
    }
}
