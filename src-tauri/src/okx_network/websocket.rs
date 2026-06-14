use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::{
    client_async_tls_with_config, connect_async,
    tungstenite::{client::IntoClientRequest, handshake::client::Response},
};

use super::{
    proxy::{effective_proxy_label, parse_http_proxy, resolve_websocket_proxy_url},
    OkxWsStream, CONNECT_TIMEOUT, OKX_USER_AGENT,
};

const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
const PROXY_RESPONSE_LIMIT: usize = 16 * 1024;

pub async fn connect_okx_websocket(
    url: &str,
    configured_proxy_url: &str,
) -> Result<(OkxWsStream, Response)> {
    let proxy_url = resolve_websocket_proxy_url(configured_proxy_url)
        .map_err(|error| anyhow!(error.to_string()))?;
    match proxy_url {
        Some(proxy_url) => connect_websocket_via_http_proxy(url, &proxy_url).await,
        None => timeout(CONNECT_TIMEOUT, connect_async(url))
            .await
            .map_err(|_| anyhow!("OKX websocket connect timeout after 12s: {url}"))?
            .map_err(Into::into),
    }
}

pub async fn diagnose_websocket(label: &str, url: &str, configured_proxy_url: &str) -> Value {
    let started = Instant::now();
    match connect_okx_websocket(url, configured_proxy_url).await {
        Ok((mut stream, response)) => {
            let _ = stream.close(None).await;
            json!({
                "label": label,
                "url": url,
                "success": true,
                "status": response.status().as_u16(),
                "latency_ms": started.elapsed().as_millis() as i64,
                "proxy": effective_proxy_label(configured_proxy_url),
            })
        }
        Err(error) => {
            tracing::warn!(
                label,
                url,
                proxy = effective_proxy_label(configured_proxy_url).as_str(),
                error = %error,
                "OKX websocket diagnostic failed"
            );
            json!({
                "label": label,
                "url": url,
                "success": false,
                "latency_ms": started.elapsed().as_millis() as i64,
                "proxy": effective_proxy_label(configured_proxy_url),
                "error": error.to_string(),
            })
        }
    }
}

async fn connect_websocket_via_http_proxy(
    url: &str,
    proxy_url: &str,
) -> Result<(OkxWsStream, Response)> {
    let request = url.into_client_request()?;
    let target_host = request
        .uri()
        .host()
        .ok_or_else(|| anyhow!("OKX websocket URL missing host: {url}"))?
        .to_string();
    let target_port = request
        .uri()
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or_else(|| anyhow!("OKX websocket URL missing port: {url}"))?;
    let proxy = parse_http_proxy(proxy_url)?;
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);

    tracing::debug!(
        proxy = proxy.display_url.as_str(),
        target = format!("{target_host}:{target_port}"),
        "opening HTTP CONNECT tunnel for OKX websocket"
    );

    let mut socket = timeout(PROXY_CONNECT_TIMEOUT, TcpStream::connect(&proxy_addr))
        .await
        .map_err(|_| anyhow!("OKX proxy connect timeout after 12s: {proxy_addr}"))?
        .with_context(|| format!("connect OKX proxy failed: {proxy_addr}"))?;

    let mut connect_request = format!(
        "CONNECT {target_host}:{target_port} HTTP/1.1\r\n\
         Host: {target_host}:{target_port}\r\n\
         User-Agent: {OKX_USER_AGENT}\r\n\
         Proxy-Connection: Keep-Alive\r\n"
    );
    if let Some(auth_header) = proxy.auth_header {
        connect_request.push_str("Proxy-Authorization: ");
        connect_request.push_str(&auth_header);
        connect_request.push_str("\r\n");
    }
    connect_request.push_str("\r\n");

    timeout(
        PROXY_CONNECT_TIMEOUT,
        socket.write_all(connect_request.as_bytes()),
    )
    .await
    .map_err(|_| anyhow!("OKX proxy CONNECT write timeout: {proxy_addr}"))?
    .with_context(|| format!("write OKX proxy CONNECT failed: {proxy_addr}"))?;

    read_proxy_connect_response(&mut socket, &proxy_addr).await?;

    client_async_tls_with_config(request, socket, None, None)
        .await
        .map_err(Into::into)
}

async fn read_proxy_connect_response(socket: &mut TcpStream, proxy_addr: &str) -> Result<()> {
    let mut response = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let read = timeout(PROXY_CONNECT_TIMEOUT, socket.read(&mut chunk))
            .await
            .map_err(|_| anyhow!("OKX proxy CONNECT response timeout: {proxy_addr}"))?
            .with_context(|| format!("read OKX proxy CONNECT response failed: {proxy_addr}"))?;
        if read == 0 {
            return Err(anyhow!(
                "OKX proxy closed connection before CONNECT response: {proxy_addr}"
            ));
        }
        response.extend_from_slice(&chunk[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > PROXY_RESPONSE_LIMIT {
            return Err(anyhow!(
                "OKX proxy CONNECT response too large: {proxy_addr}"
            ));
        }
    }

    let text = String::from_utf8_lossy(&response);
    let status_line = text.lines().next().unwrap_or_default().trim().to_string();
    if !status_line.contains(" 200 ") && !status_line.ends_with(" 200") {
        return Err(anyhow!(
            "OKX proxy CONNECT failed via {proxy_addr}: {status_line}"
        ));
    }
    Ok(())
}
