use std::path::PathBuf;

use crate::error::AppResult;

use super::{
    env_file::EnvFile,
    types::{
        ApiCredentials, AppConfig, AssistantConfig, CacheConfig, OkxConfig, TrendResearchConfig,
    },
};

#[derive(Clone)]
pub struct ConfigManager {
    env_path: PathBuf,
}

impl ConfigManager {
    pub fn new(env_path: PathBuf) -> Self {
        Self { env_path }
    }

    pub fn load(&self) -> AppResult<AppConfig> {
        let env = EnvFile::load(&self.env_path)?;
        Ok(AppConfig {
            okx: OkxConfig {
                demo: ApiCredentials {
                    api_key: env.get("OKX_DEMO_API_KEY"),
                    secret_key: env.get("OKX_DEMO_SECRET_KEY"),
                    passphrase: env.get("OKX_DEMO_PASSPHRASE"),
                },
                live: ApiCredentials {
                    api_key: env.get("OKX_LIVE_API_KEY"),
                    secret_key: env.get("OKX_LIVE_SECRET_KEY"),
                    passphrase: env.get("OKX_LIVE_PASSPHRASE"),
                },
                use_simulated: env.get_bool("OKX_USE_SIMULATED", true),
                rest_base_url: env
                    .get_non_empty("OKX_REST_BASE_URL")
                    .unwrap_or_else(|| "https://www.okx.com".to_string()),
                proxy_url: env.get("OKX_PROXY_URL"),
            },
            assistant: AssistantConfig {
                enabled: env.get_bool("AI_ASSISTANT_ENABLED", true),
                base_url: env
                    .get_non_empty("AI_ASSISTANT_BASE_URL")
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                api_key: env.get("AI_ASSISTANT_API_KEY"),
                model: env
                    .get_non_empty("AI_ASSISTANT_MODEL")
                    .unwrap_or_else(|| "gpt-4.1-mini".to_string()),
                provider_name: env
                    .get_non_empty("AI_ASSISTANT_PROVIDER")
                    .unwrap_or_else(|| "OpenAI".to_string()),
            },
            cache: CacheConfig {
                candle_cache_size: env.get_u64("CACHE_CANDLE_SIZE", 10_000),
                sync_cooldown: env.get_u64("CACHE_SYNC_COOLDOWN", 300),
                ticker_cache_ttl: env.get_u64("CACHE_TICKER_TTL", 15),
                okx_rate_limit: env.get_u64("OKX_RATE_LIMIT", 3_000),
            },
            trend_research: TrendResearchConfig {
                enabled: env.get_bool("TREND_RESEARCH_ENABLED", false),
                whitelist: parse_csv_symbols(&env.get("TREND_RESEARCH_WHITELIST")),
                feature_bar_seconds: env.get_u64("TREND_RESEARCH_FEATURE_BAR_SECONDS", 1).max(1),
                state_sync_seconds: env.get_u64("TREND_RESEARCH_STATE_SYNC_SECONDS", 30).max(5),
                book_channel: env
                    .get_non_empty("TREND_RESEARCH_BOOK_CHANNEL")
                    .unwrap_or_else(|| "books5".to_string()),
            },
            database_path: env.get_non_empty("DATABASE_PATH").map(PathBuf::from),
            api_host: env
                .get_non_empty("API_HOST")
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            api_port: env.get_u64("API_PORT", 8000).clamp(1, 65535) as u16,
            api_debug: env.get_bool("API_DEBUG", true),
        })
    }

    pub fn save_okx(&self, okx: &OkxConfig) -> AppResult<()> {
        let mut env = EnvFile::load(&self.env_path)?;
        env.set("OKX_DEMO_API_KEY", &okx.demo.api_key);
        env.set("OKX_DEMO_SECRET_KEY", &okx.demo.secret_key);
        env.set("OKX_DEMO_PASSPHRASE", &okx.demo.passphrase);
        env.set("OKX_LIVE_API_KEY", &okx.live.api_key);
        env.set("OKX_LIVE_SECRET_KEY", &okx.live.secret_key);
        env.set("OKX_LIVE_PASSPHRASE", &okx.live.passphrase);
        env.set(
            "OKX_USE_SIMULATED",
            if okx.use_simulated { "true" } else { "false" },
        );
        env.set("OKX_REST_BASE_URL", &okx.rest_base_url);
        env.set("OKX_PROXY_URL", &okx.proxy_url);
        env.save(&self.env_path)
    }

    pub fn save_assistant(&self, assistant: &AssistantConfig) -> AppResult<()> {
        let mut env = EnvFile::load(&self.env_path)?;
        env.set(
            "AI_ASSISTANT_ENABLED",
            if assistant.enabled { "true" } else { "false" },
        );
        env.set("AI_ASSISTANT_BASE_URL", &assistant.base_url);
        env.set("AI_ASSISTANT_API_KEY", &assistant.api_key);
        env.set("AI_ASSISTANT_MODEL", &assistant.model);
        env.set("AI_ASSISTANT_PROVIDER", &assistant.provider_name);
        env.save(&self.env_path)
    }

    pub fn save_trend_research(&self, trend_research: &TrendResearchConfig) -> AppResult<()> {
        let mut env = EnvFile::load(&self.env_path)?;
        env.set(
            "TREND_RESEARCH_ENABLED",
            if trend_research.enabled {
                "true"
            } else {
                "false"
            },
        );
        env.set(
            "TREND_RESEARCH_WHITELIST",
            &trend_research.whitelist.join(","),
        );
        env.set(
            "TREND_RESEARCH_FEATURE_BAR_SECONDS",
            &trend_research.feature_bar_seconds.to_string(),
        );
        env.set(
            "TREND_RESEARCH_STATE_SYNC_SECONDS",
            &trend_research.state_sync_seconds.to_string(),
        );
        env.set("TREND_RESEARCH_BOOK_CHANNEL", &trend_research.book_channel);
        env.save(&self.env_path)
    }
}

fn parse_csv_symbols(raw: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    raw.split(',')
        .filter_map(|item| {
            let normalized = item.trim().to_uppercase();
            if normalized.is_empty() || !seen.insert(normalized.clone()) {
                None
            } else {
                Some(normalized)
            }
        })
        .collect()
}
