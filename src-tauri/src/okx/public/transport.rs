use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::super::{
    normalize::build_request_path,
    outbound_record::{record_okx_outbound, OkxOutboundRecordDefaults},
    response::{okx_api_code, okx_api_message},
    retry::{network_retry_backoff, reqwest_error_chain, should_retry_network_error, RETRY_LIMIT},
};
use super::OkxPublicClient;

impl OkxPublicClient {
    pub(in crate::okx::public) async fn get_json(
        &self,
        op_key: &str,
        path: &str,
        params: &[(&str, String)],
        inst_id: Option<&str>,
    ) -> AppResult<Value> {
        let start = Instant::now();

        let request_path = build_request_path(path, params);
        let url = format!("{}{}", self.base_url, request_path);

        let mut attempt = 0;
        loop {
            self.rate_limit_wait(op_key, inst_id).await;
            let _permit = if let Some(bucket) = &self.bucket {
                Some(bucket.acquire_request_permit(op_key, inst_id).await)
            } else {
                None
            };
            tracing::debug!(
                op_key,
                path,
                request_path = request_path.as_str(),
                inst_id = inst_id.unwrap_or(""),
                attempt,
                "sending OKX public request"
            );
            let result = self.http.get(&url).send().await;
            match result {
                Ok(response) => {
                    let elapsed_ms = start.elapsed().as_millis() as i64;
                    let status = response.status();
                    let payload = response.json::<Value>().await.map_err(|error| {
                        AppError::Runtime(format!("OKX response decode failed: {error}"))
                    })?;
                    if !status.is_success() {
                        self.record(op_key, inst_id, "http_error", elapsed_ms);
                        if status.as_u16() == 429 && attempt < RETRY_LIMIT {
                            attempt += 1;
                            let backoff = Duration::from_millis(1_000 * 2u64.pow(attempt - 1));
                            tracing::warn!(
                                op_key,
                                path,
                                request_path = request_path.as_str(),
                                inst_id = inst_id.unwrap_or(""),
                                attempt,
                                backoff_ms = backoff.as_millis() as i64,
                                payload = %payload,
                                "OKX rate limited request; retrying after backoff"
                            );
                            tokio::time::sleep(backoff).await;
                            continue;
                        }
                        tracing::warn!(
                            op_key,
                            path,
                            request_path = request_path.as_str(),
                            inst_id = inst_id.unwrap_or(""),
                            status = %status,
                            payload = %payload,
                            "OKX public request failed"
                        );
                        return Err(AppError::Runtime(format!(
                            "OKX HTTP status {}: {} ({request_path})",
                            status, payload
                        )));
                    }
                    let code = match okx_api_code(&payload, "public", Some(&request_path)) {
                        Ok(code) => code,
                        Err(error) => {
                            self.record(op_key, inst_id, "api_error", elapsed_ms);
                            return Err(error);
                        }
                    };
                    if code != "0" {
                        let message = okx_api_message(&payload, "OKX API error");
                        self.record(op_key, inst_id, "api_error", elapsed_ms);
                        tracing::warn!(
                            op_key,
                            path,
                            request_path = request_path.as_str(),
                            inst_id = inst_id.unwrap_or(""),
                            code,
                            message,
                            "OKX public API returned error"
                        );
                        return Err(AppError::Runtime(format!(
                            "OKX API error {code}: {message} ({request_path})"
                        )));
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
                    return Err(AppError::Runtime(format!(
                        "OKX request failed: {} ({request_path})",
                        reqwest_error_chain(&error)
                    )));
                }
            }
        }
    }

    fn record(&self, op_key: &str, inst_id: Option<&str>, result: &str, latency_ms: i64) {
        record_okx_outbound(
            &self.timeline,
            &self.registry,
            op_key,
            inst_id,
            result,
            latency_ms,
            OkxOutboundRecordDefaults {
                rule_key: "public_ip",
                target_group: "public",
                mode: "",
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

    use crate::okx::public::OkxPublicClient;

    #[tokio::test]
    async fn public_transport_rejects_missing_top_level_code() {
        let (base_url, stop_server) = start_mock_public_server(
            json!({
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = OkxPublicClient::new_with_proxy(base_url, "direct").expect("public client");

        let error = client
            .get_json("market.ticker", "/api/v5/market/ticker", &[], None)
            .await
            .expect_err("OKX public payload without code must not be treated as success")
            .to_string();

        assert!(error.contains("code"));

        let _ = stop_server.send(());
    }

    async fn start_mock_public_server(body: String) -> (String, oneshot::Sender<()>) {
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
