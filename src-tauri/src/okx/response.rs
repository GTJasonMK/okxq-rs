use reqwest::StatusCode;
use serde_json::Value;

use crate::error::{AppError, AppResult};

pub(in crate::okx) fn parse_okx_response_body(
    api_label: &str,
    status: StatusCode,
    content_type: &str,
    body_text: &str,
) -> AppResult<Value> {
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        AppError::Runtime(format!(
            "OKX {api_label} response is not valid JSON (status {status}, content-type {}, body: {}): {error}",
            response_content_type(content_type),
            response_body_snippet(body_text)
        ))
    })
}

pub(in crate::okx) fn okx_api_code<'a>(
    payload: &'a Value,
    api_label: &str,
    request_path: Option<&str>,
) -> AppResult<&'a str> {
    payload.get("code").and_then(Value::as_str).ok_or_else(|| {
        let suffix = request_path
            .map(|path| format!(" ({path})"))
            .unwrap_or_default();
        AppError::Runtime(format!("OKX {api_label} API response missing code{suffix}"))
    })
}

pub(in crate::okx) fn okx_api_message<'a>(payload: &'a Value, default: &'a str) -> &'a str {
    payload
        .get("msg")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default)
}

pub(in crate::okx) fn response_content_type(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "<empty>"
    } else {
        trimmed
    }
}

pub(in crate::okx) fn response_body_snippet(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "<empty>".to_string();
    }

    const LIMIT: usize = 500;
    let mut snippet = normalized.chars().take(LIMIT).collect::<String>();
    if normalized.chars().count() > LIMIT {
        snippet.push('…');
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn okx_api_code_error_keeps_request_path_when_present() {
        let error = okx_api_code(
            &serde_json::json!({"data": []}),
            "public",
            Some("/api/v5/market/ticker"),
        )
        .expect_err("missing code must fail")
        .to_string();

        assert!(error.contains("OKX public API response missing code"));
        assert!(error.contains("/api/v5/market/ticker"));
    }

    #[test]
    fn response_body_snippet_collapses_whitespace_and_truncates() {
        let body = format!("{}\n{}", "x ".repeat(600), "tail");
        let snippet = response_body_snippet(&body);

        assert!(snippet.len() <= 503);
        assert!(!snippet.contains('\n'));
        assert!(snippet.ends_with('…'));
    }
}
