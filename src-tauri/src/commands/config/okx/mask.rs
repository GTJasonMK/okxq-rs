use crate::{
    config::{mask_key, ApiCredentials, OkxConfig},
    okx_network::effective_proxy_label,
};

use super::types::{MaskedCredentials, OkxConfigResponse};

pub(super) fn mask_okx_config(okx: &OkxConfig) -> OkxConfigResponse {
    OkxConfigResponse {
        demo: mask_credentials(&okx.demo),
        live: mask_credentials(&okx.live),
        use_simulated: okx.use_simulated,
        is_configured: okx.is_valid(),
        proxy_url: okx.proxy_url.clone(),
        effective_proxy_url: effective_proxy_label(&okx.proxy_url),
    }
}

fn mask_credentials(credentials: &ApiCredentials) -> MaskedCredentials {
    MaskedCredentials {
        api_key: mask_key(&credentials.api_key),
        secret_key: mask_key(&credentials.secret_key),
        passphrase: mask_key(&credentials.passphrase),
        is_configured: credentials.is_valid(),
    }
}
