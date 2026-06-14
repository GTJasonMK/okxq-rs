use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use reqwest::Url;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub(super) struct HttpProxyEndpoint {
    pub(super) display_url: String,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) auth_header: Option<String>,
}

pub fn resolve_proxy_url(configured_proxy_url: &str) -> AppResult<Option<String>> {
    let raw = configured_proxy_url.trim();
    if proxy_disabled(raw) {
        return Ok(None);
    }
    let value = if raw.is_empty() {
        proxy_from_env().unwrap_or_default()
    } else {
        raw.to_string()
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(normalize_rest_proxy_url(&value)?))
}

pub fn effective_proxy_url(configured_proxy_url: &str) -> Option<String> {
    resolve_proxy_url(configured_proxy_url).ok().flatten()
}

pub fn effective_proxy_label(configured_proxy_url: &str) -> String {
    effective_proxy_url(configured_proxy_url)
        .as_deref()
        .map(masked_proxy_url)
        .unwrap_or_default()
}

pub fn masked_proxy_url(proxy_url: &str) -> String {
    let Ok(mut parsed) = Url::parse(proxy_url) else {
        return proxy_url.to_string();
    };
    if !parsed.username().is_empty() {
        let _ = parsed.set_username("****");
    }
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("****"));
    }
    parsed.to_string()
}

pub(super) fn proxy_disabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "direct" | "none" | "off" | "no_proxy" | "noproxy" | "disabled"
    )
}

pub(super) fn resolve_websocket_proxy_url(configured_proxy_url: &str) -> AppResult<Option<String>> {
    match resolve_proxy_url(configured_proxy_url)? {
        Some(proxy_url) => Ok(Some(normalize_http_proxy_url(&proxy_url)?)),
        None => Ok(None),
    }
}

pub(super) fn parse_http_proxy(proxy_url: &str) -> Result<HttpProxyEndpoint> {
    let parsed = Url::parse(proxy_url).context("parse OKX proxy URL failed")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("OKX proxy URL missing host"))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("OKX proxy URL missing port"))?;
    let auth_header = if parsed.username().is_empty() && parsed.password().is_none() {
        None
    } else {
        let password = parsed.password().unwrap_or_default();
        let token = general_purpose::STANDARD.encode(format!("{}:{password}", parsed.username()));
        Some(format!("Basic {token}"))
    };
    Ok(HttpProxyEndpoint {
        display_url: masked_proxy_url(proxy_url),
        host,
        port,
        auth_header,
    })
}

fn proxy_from_env() -> Option<String> {
    [
        "OKX_PROXY_URL",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok())
    .map(|value| value.trim().to_string())
    .find(|value| !value.is_empty())
}

fn normalize_rest_proxy_url(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let parsed = Url::parse(&candidate)
        .map_err(|error| AppError::Validation(format!("OKX_PROXY_URL 无效: {error}")))?;
    match parsed.scheme() {
        "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h" => {}
        scheme => {
            return Err(AppError::Validation(format!(
                "OKX_PROXY_URL 代理协议不支持: {scheme}，请使用 http://、https:// 或 socks5://"
            )));
        }
    }
    if parsed.host_str().is_none() || parsed.port_or_known_default().is_none() {
        return Err(AppError::Validation(
            "OKX_PROXY_URL 必须包含代理主机和端口，例如 http://127.0.0.1:7897".to_string(),
        ));
    }
    Ok(candidate)
}

fn normalize_http_proxy_url(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let parsed = Url::parse(&candidate)
        .map_err(|error| AppError::Validation(format!("OKX_PROXY_URL 无效: {error}")))?;
    if parsed.scheme() != "http" {
        return Err(AppError::Validation(
            "OKX WebSocket 代理当前支持 HTTP CONNECT 代理，请使用 http://127.0.0.1:端口"
                .to_string(),
        ));
    }
    if parsed.host_str().is_none() || parsed.port_or_known_default().is_none() {
        return Err(AppError::Validation(
            "OKX_PROXY_URL 必须包含代理主机和端口，例如 http://127.0.0.1:7897".to_string(),
        ));
    }
    Ok(candidate)
}
