use serde_json::Value;

use crate::{
    config::ApiCredentials,
    error::{AppError, AppResult},
};

use super::super::{
    normalize::normalize_private_mode,
    state::{
        has_any_private_subscriptions_for_mode, has_private_subscriptions_for_mode,
        private_refs_mut, PrivateChannel,
    },
    RealtimeManager,
};

impl RealtimeManager {
    pub async fn subscribe_account(
        &self,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Value> {
        self.add_private_subscription(mode, credentials, PrivateChannel::Account)
            .await
    }

    pub async fn unsubscribe_account(&self, mode: &str) -> AppResult<Value> {
        self.remove_private_subscription(mode, PrivateChannel::Account)
            .await
    }

    pub async fn subscribe_orders(
        &self,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Value> {
        self.add_private_subscription(mode, credentials, PrivateChannel::Orders)
            .await
    }

    pub async fn unsubscribe_orders(&self, mode: &str) -> AppResult<Value> {
        self.remove_private_subscription(mode, PrivateChannel::Orders)
            .await
    }

    pub async fn subscribe_algo_orders(
        &self,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Value> {
        self.add_private_business_subscription(mode, credentials, PrivateChannel::AlgoOrders)
            .await
    }

    pub async fn unsubscribe_algo_orders(&self, mode: &str) -> AppResult<Value> {
        self.remove_private_business_subscription(mode, PrivateChannel::AlgoOrders)
            .await
    }

    pub async fn subscribe_fills(
        &self,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Value> {
        self.add_private_subscription(mode, credentials, PrivateChannel::Fills)
            .await
    }

    pub async fn unsubscribe_fills(&self, mode: &str) -> AppResult<Value> {
        self.remove_private_subscription(mode, PrivateChannel::Fills)
            .await
    }

    pub async fn subscribe_positions(
        &self,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Value> {
        self.add_private_subscription(mode, credentials, PrivateChannel::Positions)
            .await
    }

    pub async fn unsubscribe_positions(&self, mode: &str) -> AppResult<Value> {
        self.remove_private_subscription(mode, PrivateChannel::Positions)
            .await
    }

    async fn add_private_subscription(
        &self,
        mode: &str,
        credentials: ApiCredentials,
        channel: PrivateChannel,
    ) -> AppResult<Value> {
        let mode = normalize_private_mode(mode)?;
        if !credentials.is_valid() {
            return Err(AppError::Validation(format!(
                "{mode} OKX API 密钥未配置完整，无法订阅私有实时通道"
            )));
        }
        {
            let mut state = self.state.lock().await;
            state.private_credentials.insert(mode.clone(), credentials);
            let refs = private_refs_mut(&mut state, channel);
            *refs.entry(mode.clone()).or_insert(0) += 1;
        }
        self.restart_private_worker(&mode).await;
        Ok(self.status().await)
    }

    async fn remove_private_subscription(
        &self,
        mode: &str,
        channel: PrivateChannel,
    ) -> AppResult<Value> {
        let mode = normalize_private_mode(mode)?;
        {
            let mut state = self.state.lock().await;
            let refs = private_refs_mut(&mut state, channel);
            if let Some(count) = refs.get_mut(&mode) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    refs.remove(&mode);
                }
            }
            if !has_private_subscriptions_for_mode(&state, &mode) {
                state.private_account_cache.remove(&mode);
                state.private_account_summary_cache.remove(&mode);
                state.private_order_cache.remove(&mode);
                state.private_fill_cache.remove(&mode);
                state.private_position_cache.remove(&mode);
            }
            if !has_any_private_subscriptions_for_mode(&state, &mode) {
                state.private_credentials.remove(&mode);
            }
        }
        self.restart_private_worker(&mode).await;
        Ok(self.status().await)
    }

    async fn add_private_business_subscription(
        &self,
        mode: &str,
        credentials: ApiCredentials,
        channel: PrivateChannel,
    ) -> AppResult<Value> {
        let mode = normalize_private_mode(mode)?;
        if !credentials.is_valid() {
            return Err(AppError::Validation(format!(
                "{mode} OKX API 密钥未配置完整，无法订阅私有实时业务通道"
            )));
        }
        {
            let mut state = self.state.lock().await;
            state.private_credentials.insert(mode.clone(), credentials);
            let refs = private_refs_mut(&mut state, channel);
            *refs.entry(mode.clone()).or_insert(0) += 1;
        }
        self.restart_private_business_worker(&mode).await;
        Ok(self.status().await)
    }

    async fn remove_private_business_subscription(
        &self,
        mode: &str,
        channel: PrivateChannel,
    ) -> AppResult<Value> {
        let mode = normalize_private_mode(mode)?;
        {
            let mut state = self.state.lock().await;
            let refs = private_refs_mut(&mut state, channel);
            if let Some(count) = refs.get_mut(&mode) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    refs.remove(&mode);
                }
            }
            if !has_any_private_subscriptions_for_mode(&state, &mode) {
                state.private_credentials.remove(&mode);
            }
            if !state.private_algo_order_refs.contains_key(&mode) {
                state.private_algo_order_cache.remove(&mode);
            }
        }
        self.restart_private_business_worker(&mode).await;
        Ok(self.status().await)
    }
}
