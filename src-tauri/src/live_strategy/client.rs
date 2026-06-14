use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    okx::{OkxPrivateClient, OkxPublicClient},
};

pub async fn build_live_strategy_client(state: &AppState) -> AppResult<OkxPublicClient> {
    let cfg = state.config.read().await;
    OkxPublicClient::new_with_proxy(cfg.okx.rest_base_url.clone(), &cfg.okx.proxy_url).map(
        |client| {
            client
                .with_outbound(
                    state.okx_outbound_timeline.clone(),
                    state.okx_rate_rules.clone(),
                )
                .with_token_bucket(state.token_bucket.clone())
        },
    )
}

pub(crate) async fn build_live_strategy_private_client(
    state: &AppState,
    mode: &str,
) -> AppResult<OkxPrivateClient> {
    let cfg = state.config.read().await;
    let normalized_mode = normalize_live_private_mode(mode)?;
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
    .map(|client| {
        client
            .with_outbound(
                state.okx_outbound_timeline.clone(),
                state.okx_rate_rules.clone(),
            )
            .with_token_bucket(state.token_bucket.clone())
    })
}

fn normalize_live_private_mode(mode: &str) -> AppResult<&'static str> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "live" => Ok("live"),
        "simulated" => Ok("simulated"),
        other => Err(AppError::Validation(format!(
            "实时策略运行模式只支持 live 或 simulated，收到 {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_live_private_mode;

    #[test]
    fn live_private_client_mode_rejects_removed_aliases() {
        assert_eq!(normalize_live_private_mode("live").unwrap(), "live");
        assert_eq!(
            normalize_live_private_mode("simulated").unwrap(),
            "simulated"
        );
        for mode in ["paper", "demo", "simulation", "real", ""] {
            assert!(normalize_live_private_mode(mode).is_err());
        }
    }
}
