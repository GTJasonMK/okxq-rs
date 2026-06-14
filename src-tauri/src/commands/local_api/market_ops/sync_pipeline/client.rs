use crate::{app_state::AppState, error::AppResult, okx::OkxPublicClient};

pub(in crate::commands::local_api::market_ops) async fn okx_public_client(
    state: &AppState,
) -> AppResult<OkxPublicClient> {
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
