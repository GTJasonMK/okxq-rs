pub fn format_private_transport_error(error: &reqwest::Error, proxy_label: &str) -> String {
    let proxy_hint = if proxy_label.trim().is_empty() {
        "当前未检测到 OKX 代理；如果你的网络需要代理，请在设置中填写 OKX_PROXY_URL，或从带有 HTTP_PROXY/HTTPS_PROXY/ALL_PROXY 的环境启动应用"
            .to_string()
    } else {
        format!("当前 OKX 代理: {proxy_label}")
    };
    format!("OKX private request failed: {error}。{proxy_hint}")
}
