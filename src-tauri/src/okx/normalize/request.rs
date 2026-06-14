use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

use crate::error::{AppError, AppResult};

const DEFAULT_BASE_URL: &str = "https://www.okx.com";

type HmacSha256 = Hmac<Sha256>;

pub fn normalize_base_url(value: String) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_BASE_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn payload_data(payload: Value) -> Vec<Value> {
    payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub fn build_request_path(path: &str, params: &[(&str, String)]) -> String {
    if params.is_empty() {
        return path.to_string();
    }
    let query = params
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    }
}

pub fn sign_okx_request(
    secret_key: &str,
    timestamp: &str,
    method: &str,
    request_path: &str,
    body: &str,
) -> AppResult<String> {
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .map_err(|error| AppError::Runtime(format!("OKX signing key invalid: {error}")))?;
    mac.update(format!("{timestamp}{method}{request_path}{body}").as_bytes());
    Ok(general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
}
