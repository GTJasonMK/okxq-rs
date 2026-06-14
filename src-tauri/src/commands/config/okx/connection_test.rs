use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    config::OkxConfig,
    error::AppResult,
    okx::OkxPrivateClient,
    okx_network::{diagnose_websocket, effective_proxy_label},
};

pub(super) async fn test_okx_config(
    state: &AppState,
    okx: OkxConfig,
    public_ws_url: &str,
    business_ws_url: &str,
) -> AppResult<serde_json::Value> {
    let mode_label = if okx.use_simulated {
        "模拟盘"
    } else {
        "实盘"
    };
    tracing::info!(
        mode = okx.default_mode(),
        configured = okx.is_valid(),
        proxy = effective_proxy_label(&okx.proxy_url).as_str(),
        "testing OKX configuration"
    );

    if !okx.is_valid() {
        tracing::warn!(
            mode = okx.default_mode(),
            "OKX configuration test skipped because credentials are incomplete"
        );
        return Ok(json!({
            "success": false,
            "message": format!("{mode_label} API 密钥未配置完整，请填写 API Key、Secret Key 和 Passphrase"),
            "data": {
                "mode": mode_label,
                "private_api": false,
                "rest_success": false,
                "proxy": effective_proxy_label(&okx.proxy_url),
            }
        }));
    }

    let client = OkxPrivateClient::new_with_proxy(
        okx.rest_base_url.clone(),
        okx.current_credentials().clone(),
        okx.use_simulated,
        &okx.proxy_url,
    )?
    .with_outbound(
        state.okx_outbound_timeline.clone(),
        state.okx_rate_rules.clone(),
    )
    .with_token_bucket(state.token_bucket.clone());
    let started = std::time::Instant::now();
    let rest_result = client.get_account_config().await;
    let rest_latency_ms = started.elapsed().as_millis() as i64;
    let public_ws = diagnose_websocket("public", public_ws_url, &okx.proxy_url).await;
    let business_ws = diagnose_websocket("business", business_ws_url, &okx.proxy_url).await;
    let public_ok = public_ws
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let business_ok = business_ws
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let diagnostic = OkxConnectionDiagnostic {
        okx: &okx,
        mode_label,
        rest_latency_ms,
        public_ok,
        business_ok,
        public_ws: &public_ws,
        business_ws: &business_ws,
    };

    match rest_result {
        Ok(_) if public_ok && business_ok => okx_test_success(&diagnostic),
        Ok(_) => okx_test_websocket_failure(&diagnostic),
        Err(error) => okx_test_rest_failure(&diagnostic, &error),
    }
}

struct OkxConnectionDiagnostic<'a> {
    okx: &'a OkxConfig,
    mode_label: &'a str,
    rest_latency_ms: i64,
    public_ok: bool,
    business_ok: bool,
    public_ws: &'a Value,
    business_ws: &'a Value,
}

fn okx_test_success(diagnostic: &OkxConnectionDiagnostic<'_>) -> AppResult<Value> {
    tracing::info!(
        mode = diagnostic.okx.default_mode(),
        diagnostic.rest_latency_ms,
        proxy = effective_proxy_label(&diagnostic.okx.proxy_url).as_str(),
        "OKX configuration test passed"
    );
    Ok(json!({
        "success": true,
        "message": format!("{} 配置测试通过，REST 和 WebSocket 均可访问", diagnostic.mode_label),
        "data": {
            "mode": diagnostic.mode_label,
            "private_api": true,
            "rest_success": true,
            "endpoint": "/api/v5/account/config",
            "latency_ms": diagnostic.rest_latency_ms,
            "proxy": effective_proxy_label(&diagnostic.okx.proxy_url),
            "websocket": {
                "public": diagnostic.public_ws,
                "business": diagnostic.business_ws,
            }
        }
    }))
}

fn okx_test_websocket_failure(diagnostic: &OkxConnectionDiagnostic<'_>) -> AppResult<Value> {
    tracing::warn!(
        mode = diagnostic.okx.default_mode(),
        diagnostic.rest_latency_ms,
        public_ws_ok = diagnostic.public_ok,
        business_ws_ok = diagnostic.business_ok,
        proxy = effective_proxy_label(&diagnostic.okx.proxy_url).as_str(),
        "OKX REST test passed but websocket diagnostic failed"
    );
    Ok(json!({
        "success": false,
        "message": format!("{} REST 可访问，但 WebSocket 连接失败，请检查代理或 8443 出站网络", diagnostic.mode_label),
        "data": {
            "mode": diagnostic.mode_label,
            "private_api": true,
            "rest_success": true,
            "endpoint": "/api/v5/account/config",
            "latency_ms": diagnostic.rest_latency_ms,
            "proxy": effective_proxy_label(&diagnostic.okx.proxy_url),
            "websocket": {
                "public": diagnostic.public_ws,
                "business": diagnostic.business_ws,
            }
        }
    }))
}

fn okx_test_rest_failure(
    diagnostic: &OkxConnectionDiagnostic<'_>,
    error: &crate::error::AppError,
) -> AppResult<Value> {
    tracing::warn!(
        mode = diagnostic.okx.default_mode(),
        diagnostic.rest_latency_ms,
        public_ws_ok = diagnostic.public_ok,
        business_ws_ok = diagnostic.business_ok,
        proxy = effective_proxy_label(&diagnostic.okx.proxy_url).as_str(),
        error = %error,
        "OKX configuration test failed"
    );
    Ok(json!({
        "success": false,
        "message": format!("{} 配置测试失败: {error}", diagnostic.mode_label),
        "data": {
            "mode": diagnostic.mode_label,
            "private_api": true,
            "rest_success": false,
            "endpoint": "/api/v5/account/config",
            "latency_ms": diagnostic.rest_latency_ms,
            "proxy": effective_proxy_label(&diagnostic.okx.proxy_url),
            "websocket": {
                "public": diagnostic.public_ws,
                "business": diagnostic.business_ws,
            }
        }
    }))
}
