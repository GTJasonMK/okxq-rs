use crate::{
    app_state::AppState,
    config::ApiCredentials,
    error::{AppError, AppResult},
    okx::{OkxPrivateClient, OkxPublicClient},
};

pub(crate) fn normalize_trading_mode(mode: &str) -> AppResult<String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "live" => Ok("live".to_string()),
        "simulated" => Ok("simulated".to_string()),
        other => Err(AppError::Validation(format!(
            "交易运行模式只支持 live 或 simulated，收到 {other}"
        ))),
    }
}

pub(crate) async fn okx_client(state: &AppState) -> AppResult<OkxPublicClient> {
    let cfg = state.config.read().await;
    OkxPublicClient::new_with_proxy(cfg.okx.rest_base_url.clone(), &cfg.okx.proxy_url).map(|c| {
        c.with_outbound(
            state.okx_outbound_timeline.clone(),
            state.okx_rate_rules.clone(),
        )
        .with_token_bucket(state.token_bucket.clone())
    })
}

pub(crate) async fn okx_private_client(
    state: &AppState,
    mode: &str,
) -> AppResult<OkxPrivateClient> {
    let cfg = state.config.read().await;
    let normalized_mode = normalize_trading_mode(mode)?;
    let credentials = if normalized_mode == "live" {
        cfg.okx.live.clone()
    } else {
        cfg.okx.demo.clone()
    };
    OkxPrivateClient::new_with_proxy(
        cfg.okx.rest_base_url.clone(),
        credentials,
        normalized_mode == "simulated",
        &cfg.okx.proxy_url,
    )
    .map(|c| {
        c.with_outbound(
            state.okx_outbound_timeline.clone(),
            state.okx_rate_rules.clone(),
        )
        .with_token_bucket(state.token_bucket.clone())
    })
}

pub(crate) async fn okx_private_credentials(
    state: &AppState,
    mode: &str,
) -> AppResult<ApiCredentials> {
    let cfg = state.config.read().await;
    let normalized_mode = normalize_trading_mode(mode)?;
    let credentials = if normalized_mode == "live" {
        cfg.okx.live.clone()
    } else {
        cfg.okx.demo.clone()
    };
    if !credentials.is_valid() {
        return Err(crate::error::AppError::Validation(format!(
            "{normalized_mode} OKX API 密钥未配置完整"
        )));
    }
    Ok(credentials)
}

pub(crate) async fn okx_rest_base_url(state: &AppState) -> String {
    state.config.read().await.okx.rest_base_url.clone()
}

#[cfg(test)]
mod tests {
    use super::normalize_trading_mode;

    #[test]
    fn trading_mode_normalization_rejects_removed_aliases() {
        assert_eq!(normalize_trading_mode("live").unwrap(), "live");
        assert_eq!(normalize_trading_mode("simulated").unwrap(), "simulated");
        for mode in ["paper", "demo", "simulation", "real", "sandbox", ""] {
            assert!(normalize_trading_mode(mode).is_err());
        }
    }
}
