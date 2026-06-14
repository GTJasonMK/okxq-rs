use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    okx::response::{
        okx_api_code, okx_api_message, parse_okx_response_body,
        response_body_snippet as shared_response_body_snippet, response_content_type,
    },
};

pub(super) fn parse_private_response_body(
    status: StatusCode,
    content_type: &str,
    body_text: &str,
) -> AppResult<Value> {
    parse_okx_response_body("private", status, content_type, body_text)
}

pub(super) fn should_retry_private_http_status(method: &Method, status: StatusCode) -> bool {
    method == Method::GET && (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
}

pub(super) fn private_http_backoff(attempt: u32) -> Duration {
    Duration::from_millis(500 * 2u64.pow(attempt.saturating_sub(1)))
}

pub(super) fn private_http_error_message(
    status: StatusCode,
    content_type: &str,
    body_text: &str,
) -> String {
    format!(
        "OKX private HTTP status {status} (content-type {}, body: {})",
        response_content_type(content_type),
        response_body_snippet(body_text)
    )
}

pub(super) fn format_private_api_error(code: &str, payload: &Value) -> String {
    let message = okx_api_message(payload, "OKX private API error");
    let details = payload
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(format_private_api_error_detail)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if details.is_empty() {
        format!("OKX private API error {code}: {message}")
    } else {
        format!(
            "OKX private API error {code}: {message}; details: {}",
            details.join(" | ")
        )
    }
}

pub(super) fn validate_private_api_payload(payload: &Value) -> AppResult<()> {
    let code = okx_api_code(payload, "private", None)?;
    if code != "0" || has_item_level_failure(payload) {
        return Err(AppError::Runtime(format_private_api_error(code, payload)));
    }
    Ok(())
}

fn has_item_level_failure(payload: &Value) -> bool {
    payload
        .get("data")
        .and_then(Value::as_array)
        .map(|items| items.iter().any(has_failed_item_status))
        .unwrap_or(false)
}

fn has_failed_item_status(item: &Value) -> bool {
    value_text(item, "sCode")
        .map(|status| status != "0")
        .unwrap_or(false)
}

fn format_private_api_error_detail(item: &Value) -> Option<String> {
    let s_code = value_text(item, "sCode")
        .or_else(|| value_text(item, "code"))
        .unwrap_or_default();
    let s_msg = value_text(item, "sMsg")
        .or_else(|| value_text(item, "msg"))
        .unwrap_or_default();
    let ord_id = value_text(item, "ordId").unwrap_or_default();
    let client_order_id = value_text(item, "clOrdId").unwrap_or_default();

    if s_code.is_empty() && s_msg.is_empty() && ord_id.is_empty() && client_order_id.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !s_code.is_empty() {
        parts.push(format!("sCode={s_code}"));
    }
    if !s_msg.is_empty() {
        parts.push(format!("sMsg={s_msg}"));
    }
    if !ord_id.is_empty() {
        parts.push(format!("ordId={ord_id}"));
    }
    if !client_order_id.is_empty() {
        parts.push(format!("clOrdId={client_order_id}"));
    }
    Some(parts.join(", "))
}

fn value_text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

pub(in crate::okx::private) fn response_body_snippet(value: &str) -> String {
    shared_response_body_snippet(value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn private_response_decode_error_includes_status_content_type_and_body() {
        let error = parse_private_response_body(
            StatusCode::BAD_GATEWAY,
            "text/html; charset=utf-8",
            "<html><body>proxy bad gateway</body></html>",
        )
        .expect_err("expected non-json body to fail")
        .to_string();

        assert!(error.contains("status 502 Bad Gateway"));
        assert!(error.contains("text/html"));
        assert!(error.contains("proxy bad gateway"));
    }

    #[test]
    fn private_response_decode_accepts_okx_json_payload() {
        let payload = parse_private_response_body(
            StatusCode::OK,
            "application/json",
            r#"{"code":"0","msg":"","data":[]}"#,
        )
        .expect("expected valid json");

        assert_eq!(payload.get("code").and_then(Value::as_str), Some("0"));
    }

    #[test]
    fn private_api_payload_rejects_missing_top_level_code() {
        let error = validate_private_api_payload(&json!({
            "data": []
        }))
        .expect_err("OKX private payload without code must not be treated as success")
        .to_string();

        assert!(error.contains("code"));
    }

    #[test]
    fn private_http_retry_is_get_only_for_rate_limit_and_server_errors() {
        assert!(should_retry_private_http_status(
            &Method::GET,
            StatusCode::GATEWAY_TIMEOUT
        ));
        assert!(should_retry_private_http_status(
            &Method::GET,
            StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(!should_retry_private_http_status(
            &Method::POST,
            StatusCode::GATEWAY_TIMEOUT
        ));
        assert!(!should_retry_private_http_status(
            &Method::GET,
            StatusCode::UNAUTHORIZED
        ));
    }

    #[test]
    fn private_http_error_message_includes_html_gateway_timeout_body() {
        let message = private_http_error_message(
            StatusCode::GATEWAY_TIMEOUT,
            "text/html; charset=UTF-8",
            "<html><title>okx.com | 504: Gateway time-out</title></html>",
        );

        assert!(message.contains("504 Gateway Timeout"));
        assert!(message.contains("text/html"));
        assert!(message.contains("Gateway time-out"));
    }

    #[test]
    fn private_api_error_includes_okx_item_failure_details() {
        let payload = json!({
            "code": "1",
            "msg": "All operations failed",
            "data": [{
                "ordId": "",
                "clOrdId": "local-1",
                "sCode": "51000",
                "sMsg": "Parameter posSide error"
            }]
        });

        let message = format_private_api_error("1", &payload);

        assert!(message.contains("OKX private API error 1: All operations failed"));
        assert!(message.contains("sCode=51000"));
        assert!(message.contains("sMsg=Parameter posSide error"));
        assert!(message.contains("clOrdId=local-1"));
    }

    #[test]
    fn private_api_payload_rejects_item_failure_under_success_code() {
        let payload = json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ordId": "",
                "clOrdId": "local-1",
                "sCode": "51000",
                "sMsg": "Parameter posSide error"
            }]
        });

        let message = validate_private_api_payload(&payload)
            .expect_err("nonzero sCode must be treated as API failure")
            .to_string();

        assert!(message.contains("OKX private API error 0"));
        assert!(message.contains("sCode=51000"));
        assert!(message.contains("sMsg=Parameter posSide error"));
        assert!(message.contains("clOrdId=local-1"));
    }

    #[test]
    fn private_api_payload_allows_item_success_status() {
        let payload = json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ordId": "submitted-1",
                "clOrdId": "local-1",
                "sCode": "0",
                "sMsg": ""
            }]
        });

        validate_private_api_payload(&payload).expect("item success should pass");
    }

    #[test]
    fn private_api_payload_allows_account_rows_without_item_status() {
        let payload = json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ccy": "USDT",
                "cashBal": "100",
                "availBal": "100"
            }]
        });

        validate_private_api_payload(&payload).expect("non-trade rows without sCode should pass");
    }
}
