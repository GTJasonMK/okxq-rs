#[derive(Clone, Debug)]
pub struct OKXConcurrencySettings {
    pub okx_max_concurrency: usize,
    pub okx_public_rest_concurrency: usize,
    pub okx_private_rest_concurrency: usize,
    pub okx_trade_rest_concurrency: usize,
    pub okx_ws_control_concurrency: usize,
    pub okx_unknown_concurrency: usize,
}

impl Default for OKXConcurrencySettings {
    fn default() -> Self {
        Self {
            okx_max_concurrency: env_limit("OKXQ_OKX_MAX_CONCURRENCY", 10),
            okx_public_rest_concurrency: env_limit("OKXQ_OKX_PUBLIC_REST_CONCURRENCY", 8),
            okx_private_rest_concurrency: env_limit("OKXQ_OKX_PRIVATE_REST_CONCURRENCY", 2),
            okx_trade_rest_concurrency: env_limit("OKXQ_OKX_TRADE_REST_CONCURRENCY", 2),
            okx_ws_control_concurrency: env_limit("OKXQ_OKX_WS_CONTROL_CONCURRENCY", 1),
            okx_unknown_concurrency: env_limit("OKXQ_OKX_UNKNOWN_CONCURRENCY", 1),
        }
        .normalized()
    }
}

impl OKXConcurrencySettings {
    pub fn normalized(self) -> Self {
        Self {
            okx_max_concurrency: self.okx_max_concurrency.clamp(1, 64),
            okx_public_rest_concurrency: self.okx_public_rest_concurrency.clamp(1, 64),
            okx_private_rest_concurrency: self.okx_private_rest_concurrency.clamp(1, 32),
            okx_trade_rest_concurrency: self.okx_trade_rest_concurrency.clamp(1, 16),
            okx_ws_control_concurrency: self.okx_ws_control_concurrency.clamp(1, 8),
            okx_unknown_concurrency: self.okx_unknown_concurrency.clamp(1, 16),
        }
    }
}

fn env_limit(key: &str, default_limit: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_limit)
}
