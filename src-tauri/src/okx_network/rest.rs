use std::time::Duration;

use crate::error::{AppError, AppResult};

use super::{
    proxy::{masked_proxy_url, proxy_disabled, resolve_proxy_url},
    OKX_USER_AGENT,
};

pub fn build_okx_http_client(configured_proxy_url: &str) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(OKX_USER_AGENT)
        .timeout(Duration::from_secs(15));

    if proxy_disabled(configured_proxy_url) {
        tracing::info!("OKX REST client proxy disabled by configuration");
        builder = builder.no_proxy();
    } else if let Some(proxy_url) = resolve_proxy_url(configured_proxy_url)? {
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|error| AppError::Runtime(format!("OKX proxy invalid: {error}")))?;
        tracing::info!(
            proxy = masked_proxy_url(&proxy_url).as_str(),
            "OKX REST client proxy enabled"
        );
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|error| AppError::Runtime(error.to_string()))
}
