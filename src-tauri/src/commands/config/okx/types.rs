use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CredentialsRequest {
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OkxConfigRequest {
    pub demo: Option<CredentialsRequest>,
    pub live: Option<CredentialsRequest>,
    pub use_simulated: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaskedCredentials {
    pub api_key: String,
    pub secret_key: String,
    pub passphrase: String,
    pub is_configured: bool,
}

#[derive(Debug, Serialize)]
pub struct OkxConfigResponse {
    pub demo: MaskedCredentials,
    pub live: MaskedCredentials,
    pub use_simulated: bool,
    pub is_configured: bool,
    pub proxy_url: String,
    pub effective_proxy_url: String,
}
