use std::time::Duration;

use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

mod proxy;
mod rest;
mod websocket;

pub const OKX_PUBLIC_WS_URL: &str = "wss://ws.okx.com:8443/ws/v5/public";
pub const OKX_BUSINESS_WS_URL: &str = "wss://ws.okx.com:8443/ws/v5/business";
pub const OKX_BUSINESS_WS_URL_SIMULATED: &str =
    "wss://wspap.okx.com:8443/ws/v5/business?brokerId=9999";
pub const OKX_PRIVATE_WS_URL_LIVE: &str = "wss://ws.okx.com:8443/ws/v5/private";
pub const OKX_PRIVATE_WS_URL_SIMULATED: &str =
    "wss://wspap.okx.com:8443/ws/v5/private?brokerId=9999";

pub type OkxWsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

const OKX_USER_AGENT: &str = "okxq-rs/0.1";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);

pub use self::proxy::effective_proxy_label;
pub use self::rest::build_okx_http_client;
pub use self::websocket::{connect_okx_websocket, diagnose_websocket};

#[cfg(test)]
mod tests {
    #[test]
    fn resolve_proxy_url_can_force_direct_mode() {
        for value in ["direct", "none", "off", "no_proxy", "noproxy", "disabled"] {
            assert!(super::proxy::resolve_proxy_url(value).unwrap().is_none());
        }
    }
}
