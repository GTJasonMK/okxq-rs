use tokio::time::sleep;

use crate::config::ApiCredentials;

use super::super::{
    normalize::normalize_private_mode,
    state::{
        has_private_business_subscriptions_for_mode, has_private_subscriptions_for_mode,
        has_public_subscriptions,
    },
    RealtimeManager, WS_RECONNECT_INITIAL_DELAY, WS_RECONNECT_MAX_DELAY,
};

impl RealtimeManager {
    pub(in crate::realtime) async fn restart_worker(&self) {
        let (old_task, generation, should_spawn) = {
            let mut state = self.state.lock().await;
            state.generation = state.generation.saturating_add(1);
            state.connected = false;
            state.last_error = None;
            let old_task = state.task.take();
            let should_spawn = has_public_subscriptions(&state);
            (old_task, state.generation, should_spawn)
        };

        if let Some(task) = old_task {
            task.abort();
        }

        if !should_spawn {
            return;
        }

        let manager = self.clone();
        let task = tokio::spawn(async move {
            manager.run_public_worker(generation).await;
        });

        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.task = Some(task);
        } else {
            task.abort();
        }
    }

    pub(in crate::realtime) async fn restart_candle_worker(&self) {
        let (old_task, generation, should_spawn) = {
            let mut state = self.state.lock().await;
            state.candle_generation = state.candle_generation.saturating_add(1);
            state.candle_connected = false;
            state.candle_last_error = None;
            let old_task = state.candle_task.take();
            let should_spawn = !state.candle_refs.is_empty();
            (old_task, state.candle_generation, should_spawn)
        };

        if let Some(task) = old_task {
            task.abort();
        }

        if !should_spawn {
            return;
        }

        let manager = self.clone();
        let task = tokio::spawn(async move {
            manager.run_candle_worker(generation).await;
        });

        let mut state = self.state.lock().await;
        if state.candle_generation == generation {
            state.candle_task = Some(task);
        } else {
            task.abort();
        }
    }

    pub(in crate::realtime) async fn restart_private_worker(&self, mode: &str) {
        let Ok(mode) = normalize_private_mode(mode) else {
            return;
        };
        let (old_task, generation, should_spawn) = {
            let mut state = self.state.lock().await;
            let generation = state
                .private_generations
                .get(&mode)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            state.private_generations.insert(mode.clone(), generation);
            state.private_connected.insert(mode.clone(), false);
            state.private_last_error.remove(&mode);
            let old_task = state.private_tasks.remove(&mode);
            let should_spawn = has_private_subscriptions_for_mode(&state, &mode)
                && state
                    .private_credentials
                    .get(&mode)
                    .is_some_and(ApiCredentials::is_valid);
            (old_task, generation, should_spawn)
        };

        if let Some(task) = old_task {
            task.abort();
        }

        if !should_spawn {
            return;
        }

        let manager = self.clone();
        let task_mode = mode.clone();
        let task = tokio::spawn(async move {
            manager.run_private_worker(task_mode, generation).await;
        });

        let mut state = self.state.lock().await;
        if state.private_generations.get(&mode).copied() == Some(generation) {
            state.private_tasks.insert(mode, task);
        } else {
            task.abort();
        }
    }

    pub(in crate::realtime) async fn restart_private_business_worker(&self, mode: &str) {
        let Ok(mode) = normalize_private_mode(mode) else {
            return;
        };
        let (old_task, generation, should_spawn) = {
            let mut state = self.state.lock().await;
            let generation = state
                .private_business_generations
                .get(&mode)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            state
                .private_business_generations
                .insert(mode.clone(), generation);
            state.private_business_connected.insert(mode.clone(), false);
            state.private_business_last_error.remove(&mode);
            let old_task = state.private_business_tasks.remove(&mode);
            let should_spawn = has_private_business_subscriptions_for_mode(&state, &mode)
                && state
                    .private_credentials
                    .get(&mode)
                    .is_some_and(ApiCredentials::is_valid);
            (old_task, generation, should_spawn)
        };

        if let Some(task) = old_task {
            task.abort();
        }

        if !should_spawn {
            return;
        }

        let manager = self.clone();
        let task_mode = mode.clone();
        let task = tokio::spawn(async move {
            manager
                .run_private_business_worker(task_mode, generation)
                .await;
        });

        let mut state = self.state.lock().await;
        if state.private_business_generations.get(&mode).copied() == Some(generation) {
            state.private_business_tasks.insert(mode, task);
        } else {
            task.abort();
        }
    }

    async fn run_public_worker(self, generation: u64) {
        let mut reconnect_delay = WS_RECONNECT_INITIAL_DELAY;

        while self.should_continue(generation).await {
            let subscriptions = self.active_public_subscriptions().await;
            if subscriptions.is_empty() {
                break;
            }

            match self.run_connection(generation, subscriptions).await {
                Ok(()) => {
                    reconnect_delay = WS_RECONNECT_INITIAL_DELAY;
                }
                Err(error) => {
                    self.mark_disconnected(generation, Some(error.to_string()))
                        .await;
                    if !self.should_continue(generation).await {
                        break;
                    }
                    sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(WS_RECONNECT_MAX_DELAY);
                }
            }
        }

        self.mark_worker_finished(generation).await;
    }

    async fn run_candle_worker(self, generation: u64) {
        let mut reconnect_delay = WS_RECONNECT_INITIAL_DELAY;

        while self.should_continue_candles(generation).await {
            let subscriptions = self.active_candles().await;
            if subscriptions.is_empty() {
                break;
            }

            match self.run_candle_connection(generation, subscriptions).await {
                Ok(()) => {
                    reconnect_delay = WS_RECONNECT_INITIAL_DELAY;
                }
                Err(error) => {
                    self.mark_candles_disconnected(generation, Some(error.to_string()))
                        .await;
                    if !self.should_continue_candles(generation).await {
                        break;
                    }
                    sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(WS_RECONNECT_MAX_DELAY);
                }
            }
        }

        self.mark_candle_worker_finished(generation).await;
    }

    async fn run_private_worker(self, mode: String, generation: u64) {
        let mut reconnect_delay = WS_RECONNECT_INITIAL_DELAY;

        while self.should_continue_private(&mode, generation).await {
            let Some(credentials) = self.private_credentials(&mode).await else {
                self.mark_private_disconnected(
                    &mode,
                    generation,
                    Some("OKX API 密钥未配置完整".to_string()),
                )
                .await;
                break;
            };

            match self
                .run_private_connection(&mode, generation, credentials)
                .await
            {
                Ok(()) => {
                    reconnect_delay = WS_RECONNECT_INITIAL_DELAY;
                }
                Err(error) => {
                    self.mark_private_disconnected(&mode, generation, Some(error.to_string()))
                        .await;
                    if !self.should_continue_private(&mode, generation).await {
                        break;
                    }
                    sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(WS_RECONNECT_MAX_DELAY);
                }
            }
        }

        self.mark_private_worker_finished(&mode, generation).await;
    }

    async fn run_private_business_worker(self, mode: String, generation: u64) {
        let mut reconnect_delay = WS_RECONNECT_INITIAL_DELAY;

        while self
            .should_continue_private_business(&mode, generation)
            .await
        {
            let Some(credentials) = self.private_credentials(&mode).await else {
                self.mark_private_business_disconnected(
                    &mode,
                    generation,
                    Some("OKX API 密钥未配置完整".to_string()),
                )
                .await;
                break;
            };

            match self
                .run_private_business_connection(&mode, generation, credentials)
                .await
            {
                Ok(()) => {
                    reconnect_delay = WS_RECONNECT_INITIAL_DELAY;
                }
                Err(error) => {
                    self.mark_private_business_disconnected(
                        &mode,
                        generation,
                        Some(error.to_string()),
                    )
                    .await;
                    if !self
                        .should_continue_private_business(&mode, generation)
                        .await
                    {
                        break;
                    }
                    sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(WS_RECONNECT_MAX_DELAY);
                }
            }
        }

        self.mark_private_business_worker_finished(&mode, generation)
            .await;
    }
}
