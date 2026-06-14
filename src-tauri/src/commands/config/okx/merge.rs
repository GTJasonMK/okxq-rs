use crate::{
    config::{sanitize_secret_value, ApiCredentials, OkxConfig},
    error::AppResult,
};

use super::types::{CredentialsRequest, OkxConfigRequest};

pub(super) fn merge_okx_config(
    incoming: Option<&OkxConfigRequest>,
    existing: &OkxConfig,
) -> AppResult<OkxConfig> {
    let Some(req) = incoming else {
        return Ok(existing.clone());
    };
    Ok(OkxConfig {
        demo: merge_credentials("demo", req.demo.as_ref(), &existing.demo)?,
        live: merge_credentials("live", req.live.as_ref(), &existing.live)?,
        use_simulated: req.use_simulated,
        rest_base_url: existing.rest_base_url.clone(),
        proxy_url: req
            .proxy_url
            .as_deref()
            .map(str::trim)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| existing.proxy_url.clone()),
    })
}

fn merge_credentials(
    prefix: &str,
    incoming: Option<&CredentialsRequest>,
    existing: &ApiCredentials,
) -> AppResult<ApiCredentials> {
    let empty = CredentialsRequest {
        api_key: None,
        secret_key: None,
        passphrase: None,
    };
    let req = incoming.unwrap_or(&empty);
    Ok(ApiCredentials {
        api_key: sanitize_secret_value(
            &format!("{prefix}.api_key"),
            req.api_key.as_deref().unwrap_or_default(),
            &existing.api_key,
        )?,
        secret_key: sanitize_secret_value(
            &format!("{prefix}.secret_key"),
            req.secret_key.as_deref().unwrap_or_default(),
            &existing.secret_key,
        )?,
        passphrase: sanitize_secret_value(
            &format!("{prefix}.passphrase"),
            req.passphrase.as_deref().unwrap_or_default(),
            &existing.passphrase,
        )?,
    })
}
