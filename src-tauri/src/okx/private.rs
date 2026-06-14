use std::sync::Arc;

use crate::{
    config::ApiCredentials,
    error::{AppError, AppResult},
    okx_network::{build_okx_http_client, effective_proxy_label},
    okx_outbound::{OKXOutboundTimelineStore, OKXRateRuleRegistry},
    token_bucket::SharedTokenBucketRegistry,
};

use super::normalize::normalize_base_url;

mod account;
mod order;
mod params;
mod response;
mod trade;
mod transport;

pub use self::order::OkxAttachedAlgoOrder;
pub(crate) use self::order::{normalized_order_type, order_type_requires_price};

#[derive(Clone)]
pub struct OkxPrivateClient {
    base_url: String,
    credentials: ApiCredentials,
    simulated: bool,
    http: reqwest::Client,
    proxy_label: String,
    timeline: Option<Arc<OKXOutboundTimelineStore>>,
    registry: Option<Arc<OKXRateRuleRegistry>>,
    bucket: Option<SharedTokenBucketRegistry>,
}

impl OkxPrivateClient {
    pub fn new_with_proxy(
        base_url: impl Into<String>,
        credentials: ApiCredentials,
        simulated: bool,
        proxy_url: &str,
    ) -> AppResult<Self> {
        if !credentials.is_valid() {
            return Err(AppError::Validation("OKX API 密钥未配置完整".to_string()));
        }
        let base_url = normalize_base_url(base_url.into());
        let http = build_okx_http_client(proxy_url)?;
        let proxy_label = effective_proxy_label(proxy_url);
        Ok(Self {
            base_url,
            credentials,
            simulated,
            http,
            proxy_label,
            timeline: None,
            registry: None,
            bucket: None,
        })
    }

    /// 注入限流时间线和规则注册表，用于记录 OKX 出站请求
    pub fn with_outbound(
        mut self,
        timeline: Arc<OKXOutboundTimelineStore>,
        registry: Arc<OKXRateRuleRegistry>,
    ) -> Self {
        self.timeline = Some(timeline);
        self.registry = Some(registry);
        self
    }

    /// 注入令牌桶限流器，用于主动速率控制
    pub fn with_token_bucket(mut self, bucket: SharedTokenBucketRegistry) -> Self {
        self.bucket = Some(bucket);
        self
    }
}
