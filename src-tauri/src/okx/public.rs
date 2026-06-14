use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    okx_network::build_okx_http_client,
    okx_outbound::{OKXOutboundTimelineStore, OKXRateRuleRegistry},
    token_bucket::SharedTokenBucketRegistry,
};

use super::normalize::{
    normalize_base_url, normalize_funding_rate, normalize_orderbook, normalize_trade,
    parse_okx_candle, payload_data,
};
use super::OkxCandle;

mod transport;

const HISTORICAL_MARKET_DATA_PATH: &str = "/api/v5/public/market-data-history";
const MAX_HISTORICAL_ZIP_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct OkxHistoricalDownloadLink {
    pub url: String,
    pub filename: String,
    pub date_ts: Option<i64>,
    pub size_mb: Option<f64>,
}

#[derive(Clone)]
pub struct OkxPublicClient {
    pub(in crate::okx::public) base_url: String,
    pub(in crate::okx::public) http: reqwest::Client,
    pub(in crate::okx::public) timeline: Option<Arc<OKXOutboundTimelineStore>>,
    pub(in crate::okx::public) registry: Option<Arc<OKXRateRuleRegistry>>,
    pub(in crate::okx::public) bucket: Option<SharedTokenBucketRegistry>,
}

impl OkxPublicClient {
    pub fn new_with_proxy(base_url: impl Into<String>, proxy_url: &str) -> AppResult<Self> {
        let base_url = normalize_base_url(base_url.into());
        let http = build_okx_http_client(proxy_url)?;
        Ok(Self {
            base_url,
            http,
            timeline: None,
            registry: None,
            bucket: None,
        })
    }

    /// 注入限流时间线和规则注册表，用于记录 OKX 出站请求
    pub fn with_outbound(
        mut self,
        timeline: Arc<OKXOutboundTimelineStore>,
        registry: Arc<OKXRateRuleRegistry>,
    ) -> Self {
        self.timeline = Some(timeline);
        self.registry = Some(registry);
        self
    }

    /// 注入令牌桶限流器，用于主动速率控制
    pub fn with_token_bucket(mut self, bucket: SharedTokenBucketRegistry) -> Self {
        self.bucket = Some(bucket);
        self
    }

    pub async fn get_instruments(&self, inst_type: &str) -> AppResult<Vec<Value>> {
        let payload = self
            .get_json(
                "public.instruments",
                "/api/v5/public/instruments",
                &[("instType", inst_type.to_uppercase())],
                None,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_ticker(&self, inst_id: &str) -> AppResult<Value> {
        let payload = self
            .get_json(
                "market.ticker",
                "/api/v5/market/ticker",
                &[("instId", inst_id.to_string())],
                Some(inst_id),
            )
            .await?;
        first_public_data_item(payload, "ticker")
    }

    pub async fn get_candles(
        &self,
        inst_id: &str,
        timeframe: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
        history: bool,
    ) -> AppResult<Vec<OkxCandle>> {
        let (op_key, endpoint) = if history {
            ("market.history_candles", "/api/v5/market/history-candles")
        } else {
            ("market.candles", "/api/v5/market/candles")
        };
        let bar = super::okx_bar(timeframe);
        let mut params = vec![
            ("instId", inst_id.to_string()),
            ("bar", bar),
            ("limit", limit.clamp(1, 300).to_string()),
        ];
        if let Some(before) = before.filter(|item| !item.is_empty()) {
            params.push(("before", before));
        }
        if let Some(after) = after.filter(|item| !item.is_empty()) {
            params.push(("after", after));
        }
        let payload = self
            .get_json(op_key, endpoint, &params, Some(inst_id))
            .await?;
        let mut candles = payload_data(payload)
            .into_iter()
            .filter_map(parse_okx_candle)
            .collect::<Vec<_>>();
        candles.sort_by_key(|item| item.timestamp);
        Ok(candles)
    }

    pub async fn get_trades(&self, inst_id: &str, limit: u32) -> AppResult<Vec<Value>> {
        let payload = self
            .get_json(
                "market.trades",
                "/api/v5/market/trades",
                &[
                    ("instId", inst_id.to_string()),
                    ("limit", limit.clamp(1, 500).to_string()),
                ],
                Some(inst_id),
            )
            .await?;
        Ok(payload_data(payload)
            .into_iter()
            .filter_map(|item| normalize_trade(item, inst_id))
            .collect())
    }

    pub async fn get_funding_rate(&self, inst_id: &str) -> AppResult<Value> {
        let payload = self
            .get_json(
                "market.funding_rate",
                "/api/v5/public/funding-rate",
                &[("instId", inst_id.to_string())],
                Some(inst_id),
            )
            .await?;
        let item = first_public_data_item(payload, "funding-rate")?;
        normalize_funding_rate(item).ok_or_else(|| {
            AppError::Runtime(format!("OKX funding-rate 响应无法规范化: instId={inst_id}"))
        })
    }

    pub async fn get_orderbook(&self, inst_id: &str, size: u32) -> AppResult<Value> {
        if size > 400 {
            return self.get_orderbook_full(inst_id, size).await;
        }
        let payload = self
            .get_json(
                "market.books",
                "/api/v5/market/books",
                &[
                    ("instId", inst_id.to_string()),
                    ("sz", size.clamp(1, 400).to_string()),
                ],
                Some(inst_id),
            )
            .await?;
        let item = first_public_data_item(payload, "orderbook")?;
        normalize_orderbook(item, inst_id).ok_or_else(|| {
            AppError::Runtime(format!("OKX orderbook 响应无法规范化: instId={inst_id}"))
        })
    }

    pub async fn get_orderbook_full(&self, inst_id: &str, size: u32) -> AppResult<Value> {
        let payload = self
            .get_json(
                "market.books_full",
                "/api/v5/market/books-full",
                &[
                    ("instId", inst_id.to_string()),
                    ("sz", size.clamp(1, 5000).to_string()),
                ],
                Some(inst_id),
            )
            .await?;
        let item = first_public_data_item(payload, "orderbook-full")?;
        normalize_orderbook(item, inst_id).ok_or_else(|| {
            AppError::Runtime(format!(
                "OKX orderbook-full 响应无法规范化: instId={inst_id}"
            ))
        })
    }

    pub async fn get_historical_market_data_links(
        &self,
        module: &str,
        inst_type: &str,
        inst_ids: &[String],
        start_ts: i64,
        end_ts: i64,
        date_aggr_type: &str,
    ) -> AppResult<Vec<OkxHistoricalDownloadLink>> {
        let inst_type = inst_type.trim().to_uppercase();
        let mut params = vec![
            ("module", module.to_string()),
            ("instType", inst_type.clone()),
            (
                "dateAggrType",
                normalize_historical_date_aggr_type(date_aggr_type),
            ),
            ("begin", start_ts.to_string()),
            ("end", end_ts.to_string()),
        ];
        if inst_type == "SPOT" {
            params.push(("instIdList", historical_symbol_query(inst_ids, true)));
        } else {
            params.push(("instFamilyList", historical_symbol_query(inst_ids, false)));
        }
        let payload = self
            .get_json(
                "public.market_data_history",
                HISTORICAL_MARKET_DATA_PATH,
                &params,
                inst_ids.first().map(String::as_str),
            )
            .await?;
        Ok(extract_historical_links(&payload))
    }

    pub async fn download_historical_zip(&self, url: &str) -> AppResult<Vec<u8>> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|error| AppError::Validation(format!("无效 OKX 历史 zip URL: {error}")))?;
        let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
        if parsed.scheme() != "https" || host != "static.okx.com" {
            return Err(AppError::Validation(format!(
                "已拒绝非 OKX 静态历史数据 URL: {url}"
            )));
        }

        let response = self
            .http
            .get(parsed)
            .header("User-Agent", "okxq-rs/1.0 historical-gap-repair")
            .send()
            .await
            .map_err(|error| AppError::Runtime(format!("OKX 历史 zip 下载失败: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Runtime(format!(
                "OKX 历史 zip 下载 HTTP 状态异常: {status}"
            )));
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|error| AppError::Runtime(format!("OKX 历史 zip 读取失败: {error}")))?;
            if bytes.len().saturating_add(chunk.len()) > MAX_HISTORICAL_ZIP_BYTES {
                return Err(AppError::Runtime(format!(
                    "OKX 历史 zip 超过大小限制 {} MB",
                    MAX_HISTORICAL_ZIP_BYTES / 1024 / 1024
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn normalize_historical_date_aggr_type(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("monthly") {
        "monthly".to_string()
    } else {
        "daily".to_string()
    }
}

fn historical_symbol_query(inst_ids: &[String], spot: bool) -> String {
    let values = inst_ids
        .iter()
        .map(|inst_id| {
            let mut value = inst_id.trim().to_uppercase();
            if !spot && value.ends_with("-SWAP") {
                value.truncate(value.len() - 5);
            }
            value
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        "ANY".to_string()
    } else {
        values.join(",")
    }
}

fn first_public_data_item(payload: Value, operation: &str) -> AppResult<Value> {
    payload_data(payload)
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Runtime(format!("OKX {operation}响应缺少 data[0]")))
}

fn extract_historical_links(payload: &Value) -> Vec<OkxHistoricalDownloadLink> {
    let data = payload.get("data").unwrap_or(&Value::Null);
    let groups = match data {
        Value::Array(items) => items.iter().collect::<Vec<_>>(),
        Value::Object(_) => vec![data],
        _ => Vec::new(),
    };
    let mut links = Vec::new();
    for group in groups {
        let Some(details) = group.get("details").and_then(Value::as_array) else {
            continue;
        };
        for detail in details {
            let group_details = detail
                .get("groupDetails")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![detail.clone()]);
            for item in group_details {
                let url = item
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim();
                let filename = item
                    .get("filename")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim();
                if url.is_empty() || filename.is_empty() {
                    continue;
                }
                links.push(OkxHistoricalDownloadLink {
                    url: url.to_string(),
                    filename: filename.to_string(),
                    date_ts: item
                        .get("dateTs")
                        .and_then(Value::as_str)
                        .and_then(|value| value.parse::<i64>().ok()),
                    size_mb: item
                        .get("sizeMB")
                        .and_then(Value::as_str)
                        .and_then(|value| value.parse::<f64>().ok()),
                });
            }
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::OkxPublicClient;

    #[tokio::test]
    async fn orderbook_rejects_success_payload_without_book_item() {
        let (base_url, stop_server) = start_mock_public_server(
            json!({
                "code": "0",
                "msg": "",
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = OkxPublicClient::new_with_proxy(base_url, "direct").expect("public client");

        let error = client
            .get_orderbook("BTC-USDT-SWAP", 1)
            .await
            .expect_err("empty OKX orderbook data must not become an empty success object")
            .to_string();

        assert!(error.contains("缺少 data[0]"));

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn ticker_rejects_success_payload_without_ticker_item() {
        let (base_url, stop_server) = start_mock_public_server(
            json!({
                "code": "0",
                "msg": "",
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = OkxPublicClient::new_with_proxy(base_url, "direct").expect("public client");

        let error = client
            .get_ticker("BTC-USDT-SWAP")
            .await
            .expect_err("empty OKX ticker data must not become an empty success object")
            .to_string();

        assert!(error.contains("缺少 data[0]"));

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn funding_rate_rejects_success_payload_without_funding_item() {
        let (base_url, stop_server) = start_mock_public_server(
            json!({
                "code": "0",
                "msg": "",
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = OkxPublicClient::new_with_proxy(base_url, "direct").expect("public client");

        let error = client
            .get_funding_rate("BTC-USDT-SWAP")
            .await
            .expect_err("empty OKX funding data must not become an empty success object")
            .to_string();

        assert!(error.contains("缺少 data[0]"));

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
