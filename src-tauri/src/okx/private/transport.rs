use std::time::Instant;

use reqwest::{
    header::{ACCEPT_ENCODING, CONTENT_TYPE},
    Method,
};
use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    okx::normalize::{build_request_path, format_private_transport_error, sign_okx_request},
};

use super::{
    super::{
        outbound_record::{record_okx_outbound, OkxOutboundRecordDefaults},
        retry::{network_retry_backoff, should_retry_network_error, RETRY_LIMIT},
    },
    response::{
        parse_private_response_body, private_http_backoff, private_http_error_message,
        response_body_snippet, should_retry_private_http_status, validate_private_api_payload,
    },
    OkxPrivateClient,
};

impl OkxPrivateClient {
    pub(in crate::okx::private) async fn get_json(
        &self,
        op_key: &str,
        path: &str,
        params: &[(&str, String)],
        inst_id: Option<&str>,
    ) -> AppResult<Value> {
        self.send_request(op_key, Method::GET, path, params, None, inst_id)
            .await
    }

    pub async fn post_json(
        &self,
        op_key: &str,
        path: &str,
        body: &Value,
        inst_id: Option<&str>,
    ) -> AppResult<Value> {
        self.send_request(op_key, Method::POST, path, &[], Some(body), inst_id)
            .await
    }

    async fn send_request(
        &self,
        op_key: &str,
        method: Method,
        path: &str,
        params: &[(&str, String)],
        body: Option<&Value>,
        inst_id: Option<&str>,
    ) -> AppResult<Value> {
        let start = Instant::now();

        let request_path = build_request_path(path, params);
        let body_str = match body {
            Some(value) => serde_json::to_string(value)
                .map_err(|error| AppError::Runtime(format!("JSON body 序列化失败: {error}")))?,
            None => String::new(),
        };
        let url = format!("{}{}", self.base_url, request_path);

        let mut attempt = 0;
        loop {
            self.rate_limit_wait(op_key, inst_id).await;
            let _permit = if let Some(bucket) = &self.bucket {
                Some(bucket.acquire_request_permit(op_key, inst_id).await)
            } else {
                None
            };
            let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let signature = sign_okx_request(
                &self.credentials.secret_key,
                &timestamp,
                method.as_str(),
                &request_path,
                &body_str,
            )?;
            let mut request = self
                .http
                .request(method.clone(), &url)
                .header("OK-ACCESS-KEY", &self.credentials.api_key)
                .header("OK-ACCESS-SIGN", &signature)
                .header("OK-ACCESS-TIMESTAMP", &timestamp)
                .header("OK-ACCESS-PASSPHRASE", &self.credentials.passphrase)
                .header(ACCEPT_ENCODING, "identity");
            if self.simulated {
                request = request.header("x-simulated-trading", "1");
            }
            if body.is_some() {
                request = request
                    .header("Content-Type", "application/json")
                    .body(body_str.clone());
            }
            let result = request.send().await;
            match result {
                Ok(response) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    let status = response.status();
                    let content_type = response
                        .headers()
                        .get(CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let body_text = match response.text().await {
                        Ok(value) => value,
                        Err(error) => {
                            self.record(op_key, inst_id, "read_error", elapsed_ms);
                            return Err(AppError::Runtime(format!(
                                "OKX private response read failed: {error}"
                            )));
                        }
                    };
                    if !status.is_success() {
                        if should_retry_private_http_status(&method, status)
                            && attempt < RETRY_LIMIT
                        {
                            attempt += 1;
                            let backoff = private_http_backoff(attempt);
                            tracing::warn!(
                                op_key,
                                path,
                                request_path = request_path.as_str(),
                                inst_id = inst_id.unwrap_or(""),
                                status = %status,
                                attempt,
                                backoff_ms = backoff.as_millis() as i64,
                                body = %response_body_snippet(&body_text),
                                "OKX private GET returned retryable HTTP status; retrying"
                            );
                            tokio::time::sleep(backoff).await;
                            continue;
                        }
                        self.record(op_key, inst_id, "http_error", elapsed_ms);
                        return Err(AppError::Runtime(private_http_error_message(
                            status,
                            &content_type,
                            &body_text,
                        )));
                    }
                    let payload =
                        match parse_private_response_body(status, &content_type, &body_text) {
                            Ok(value) => value,
                            Err(error) => {
                                self.record(op_key, inst_id, "decode_error", elapsed_ms);
                                return Err(error);
                            }
                        };
                    if let Err(error) = validate_private_api_payload(&payload) {
                        self.record(op_key, inst_id, "api_error", elapsed_ms);
                        return Err(error);
                    }
                    self.record(op_key, inst_id, "ok", elapsed_ms);
                    return Ok(payload);
                }
                Err(error) => {
                    if should_retry_network_error(&error, attempt) {
                        attempt += 1;
                        tokio::time::sleep(network_retry_backoff(attempt)).await;
                        continue;
                    }
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    self.record(op_key, inst_id, "network_error", elapsed_ms);
                    return Err(AppError::Runtime(format_private_transport_error(
                        &error,
                        &self.proxy_label,
                    )));
                }
            }
        }
    }

    fn record(&self, op_key: &str, inst_id: Option<&str>, result: &str, latency_ms: i64) {
        let default_target_group = if op_key.starts_with("trade")
            || op_key.contains("place_order")
            || op_key.contains("cancel_order")
        {
            "trade"
        } else {
            "private"
        };
        let mode = if self.simulated { "sim" } else { "" };
        record_okx_outbound(
            &self.timeline,
            &self.registry,
            op_key,
            inst_id,
            result,
            latency_ms,
            OkxOutboundRecordDefaults {
                rule_key: "private_user",
                target_group: default_target_group,
                mode,
            },
        );
    }

    async fn rate_limit_wait(&self, op_key: &str, inst_id: Option<&str>) {
        if let Some(bucket) = &self.bucket {
            bucket.acquire(op_key, inst_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::*;
    use crate::config::ApiCredentials;

    #[tokio::test]
    async fn private_post_rejects_okx_item_failure_under_success_code() {
        let (base_url, stop_server) = start_mock_private_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "ordId": "",
                    "clOrdId": "local-1",
                    "sCode": "51000",
                    "sMsg": "Parameter posSide error"
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client(base_url);

        let error = client
            .post_json(
                "trade.place_order",
                "/api/v5/trade/order",
                &json!({"instId": "BTC-USDT-SWAP"}),
                Some("BTC-USDT-SWAP"),
            )
            .await
            .expect_err("OKX item-level failure must not be treated as success");

        let message = error.to_string();
        assert!(message.contains("OKX private API error 0"));
        assert!(message.contains("sCode=51000"));
        assert!(message.contains("sMsg=Parameter posSide error"));
        assert!(message.contains("clOrdId=local-1"));

        let _ = stop_server.send(());
    }

    fn test_client(base_url: String) -> OkxPrivateClient {
        OkxPrivateClient::new_with_proxy(
            base_url,
            ApiCredentials {
                api_key: "api-key".to_string(),
                secret_key: "secret-key".to_string(),
                passphrase: "passphrase".to_string(),
            },
            true,
            "direct",
        )
        .expect("test client")
    }

    async fn start_mock_private_server(body: String) -> (String, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock server should bind");
        let addr = listener.local_addr().expect("mock server address");
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let Ok((mut stream, _)) = accepted else {
                            break;
                        };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let mut request = vec![0_u8; 4096];
                            let _ = stream.read(&mut request).await.unwrap_or(0);
                            let response = format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(response.as_bytes()).await;
                            let _ = stream.shutdown().await;
                        });
                    }
                }
            }
        });
        (format!("http://{addr}"), stop_tx)
    }
}
