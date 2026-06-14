use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiCredentials {
    pub api_key: String,
    pub secret_key: String,
    pub passphrase: String,
}

impl ApiCredentials {
    fn empty() -> Self {
        Self {
            api_key: String::new(),
            secret_key: String::new(),
            passphrase: String::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.api_key.is_empty() && !self.secret_key.is_empty() && !self.passphrase.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OkxConfig {
    pub demo: ApiCredentials,
    pub live: ApiCredentials,
    pub use_simulated: bool,
    pub rest_base_url: String,
    pub proxy_url: String,
}

impl Default for OkxConfig {
    fn default() -> Self {
        Self {
            demo: ApiCredentials::empty(),
            live: ApiCredentials::empty(),
            use_simulated: true,
            rest_base_url: "https://www.okx.com".to_string(),
            proxy_url: String::new(),
        }
    }
}

impl OkxConfig {
    pub fn current_credentials(&self) -> &ApiCredentials {
        if self.use_simulated {
            &self.demo
        } else {
            &self.live
        }
    }

    pub fn is_valid(&self) -> bool {
        self.current_credentials().is_valid()
    }

    pub fn default_mode(&self) -> &'static str {
        if self.use_simulated {
            "simulated"
        } else {
            "live"
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub provider_name: String,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4.1-mini".to_string(),
            provider_name: "OpenAI".to_string(),
        }
    }
}

impl AssistantConfig {
    pub fn is_configured(&self) -> bool {
        self.enabled
            && !self.base_url.trim().is_empty()
            && !self.api_key.trim().is_empty()
            && !self.model.trim().is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendResearchConfig {
    pub enabled: bool,
    pub whitelist: Vec<String>,
    pub feature_bar_seconds: u64,
    pub state_sync_seconds: u64,
    pub book_channel: String,
}

impl Default for TrendResearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            whitelist: Vec::new(),
            feature_bar_seconds: 1,
            state_sync_seconds: 30,
            book_channel: "books5".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    pub candle_cache_size: u64,
    pub sync_cooldown: u64,
    pub ticker_cache_ttl: u64,
    pub okx_rate_limit: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            candle_cache_size: 10_000,
            sync_cooldown: 300,
            ticker_cache_ttl: 15,
            okx_rate_limit: 3_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub okx: OkxConfig,
    pub assistant: AssistantConfig,
    pub cache: CacheConfig,
    pub trend_research: TrendResearchConfig,
    pub database_path: Option<PathBuf>,
    pub api_host: String,
    pub api_port: u16,
    pub api_debug: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            okx: OkxConfig::default(),
            assistant: AssistantConfig::default(),
            cache: CacheConfig::default(),
            trend_research: TrendResearchConfig::default(),
            database_path: None,
            api_host: "127.0.0.1".to_string(),
            api_port: 8000,
            api_debug: true,
        }
    }
}
