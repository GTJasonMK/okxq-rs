use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    okx::normalize::payload_data,
};

use super::{
    params::{
        normalized_required_inst_id, optional_inst_filter_params, push_optional_uppercase_param,
    },
    OkxPrivateClient,
};

impl OkxPrivateClient {
    pub async fn get_account_balance(&self) -> AppResult<Vec<Value>> {
        let payload = self
            .get_json("account.balance", "/api/v5/account/balance", &[], None)
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_positions(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
    ) -> AppResult<Vec<Value>> {
        let params = optional_inst_filter_params(inst_type, inst_id);
        let payload = self
            .get_json(
                "account.positions",
                "/api/v5/account/positions",
                &params,
                inst_id,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_max_size(&self, inst_id: &str, td_mode: &str) -> AppResult<Vec<Value>> {
        let params = vec![
            ("instId", inst_id.trim().to_uppercase()),
            ("tdMode", td_mode.trim().to_string()),
        ];
        let payload = self
            .get_json(
                "account.max_size",
                "/api/v5/account/max-size",
                &params,
                Some(inst_id),
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_leverage(&self, inst_id: &str, mgn_mode: &str) -> AppResult<Vec<Value>> {
        let params = vec![
            ("instId", inst_id.trim().to_uppercase()),
            ("mgnMode", mgn_mode.trim().to_string()),
        ];
        let payload = self
            .get_json(
                "account.get_leverage",
                "/api/v5/account/leverage-info",
                &params,
                Some(inst_id),
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_account_config(&self) -> AppResult<Value> {
        let payload = self
            .get_json("account.config", "/api/v5/account/config", &[], None)
            .await?;
        first_account_result(payload, "账户配置")
    }

    pub async fn get_trade_fee(
        &self,
        inst_type: &str,
        inst_id: &str,
        inst_family: &str,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![("instType", inst_type.trim().to_uppercase())];
        push_optional_uppercase_param(&mut params, "instId", Some(inst_id));
        push_optional_uppercase_param(&mut params, "instFamily", Some(inst_family));
        let payload = self
            .get_json(
                "account.trade_fee",
                "/api/v5/account/trade-fee",
                &params,
                if inst_id.trim().is_empty() {
                    None
                } else {
                    Some(inst_id)
                },
            )
            .await?;
        Ok(payload_data(payload))
    }

    /// 获取最大可交易数量
    pub async fn get_max_avail_size(&self, inst_id: &str, td_mode: &str) -> AppResult<Value> {
        let params = vec![
            ("instId", inst_id.trim().to_uppercase()),
            ("tdMode", td_mode.trim().to_string()),
        ];
        let payload = self
            .get_json(
                "account.max_avail_size",
                "/api/v5/account/max-avail-size",
                &params,
                Some(inst_id),
            )
            .await?;
        first_account_result(payload, "最大可交易数量")
    }

    /// 设置杠杆倍数
    pub async fn set_leverage(
        &self,
        inst_id: &str,
        lever: &str,
        mgn_mode: &str,
        pos_side: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "设置杠杆")?;
        let lever = positive_decimal_text(lever, "lever")?;
        let mgn_mode = normalized_margin_mode(mgn_mode)?;
        let pos_side = normalized_pos_side(pos_side)?;
        let mut body = json!({
            "instId": inst_id.as_str(),
            "lever": lever,
            "mgnMode": mgn_mode,
        });
        if let Some(pos_side) = pos_side {
            body["posSide"] = json!(pos_side);
        }
        self.post_json(
            "account.set_leverage",
            "/api/v5/account/set-leverage",
            &body,
            Some(&inst_id),
        )
        .await
    }

    /// 设置持仓模式
    pub async fn set_position_mode(&self, pos_mode: &str) -> AppResult<Value> {
        let pos_mode = normalized_position_mode(pos_mode)?;
        let body = json!({ "posMode": pos_mode });
        self.post_json(
            "account.set_position_mode",
            "/api/v5/account/set-position-mode",
            &body,
            None,
        )
        .await
    }
}

fn first_account_result(payload: Value, operation: &str) -> AppResult<Value> {
    payload_data(payload)
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Runtime(format!("OKX {operation}响应缺少 data[0]")))
}

fn positive_decimal_text<'a>(value: &'a str, field: &str) -> AppResult<&'a str> {
    let trimmed = value.trim();
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| AppError::Validation(format!("OKX 设置杠杆参数 {field} 必须为正数")))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(trimmed)
    } else {
        Err(AppError::Validation(format!(
            "OKX 设置杠杆参数 {field} 必须为正数"
        )))
    }
}

fn normalized_margin_mode(mgn_mode: &str) -> AppResult<&'static str> {
    match mgn_mode.trim().to_ascii_lowercase().as_str() {
        "cross" => Ok("cross"),
        "isolated" => Ok("isolated"),
        _ => Err(AppError::Validation(
            "OKX 设置杠杆参数 mgnMode 必须为 cross/isolated".to_string(),
        )),
    }
}

fn normalized_pos_side(pos_side: &str) -> AppResult<Option<&'static str>> {
    match pos_side.trim().to_ascii_lowercase().as_str() {
        "" => Ok(None),
        "long" => Ok(Some("long")),
        "short" => Ok(Some("short")),
        _ => Err(AppError::Validation(
            "OKX 设置杠杆参数 posSide 必须为空或为 long/short".to_string(),
        )),
    }
}

fn normalized_position_mode(pos_mode: &str) -> AppResult<&'static str> {
    match pos_mode.trim().to_ascii_lowercase().as_str() {
        "net_mode" => Ok("net_mode"),
        "long_short_mode" => Ok("long_short_mode"),
        _ => Err(AppError::Validation(
            "OKX 设置持仓模式参数 posMode 必须为 net_mode/long_short_mode".to_string(),
        )),
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

    use crate::{config::ApiCredentials, okx::private::OkxPrivateClient};

    fn test_client() -> OkxPrivateClient {
        OkxPrivateClient::new_with_proxy(
            "http://127.0.0.1:1",
            ApiCredentials {
                api_key: "api-key".to_string(),
                secret_key: "secret-key".to_string(),
                passphrase: "passphrase".to_string(),
            },
            true,
            "",
        )
        .expect("test client")
    }

    fn test_client_with_base_url(base_url: String) -> OkxPrivateClient {
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

    #[tokio::test]
    async fn max_avail_size_rejects_success_payload_without_result_item() {
        let (base_url, stop_server) = start_mock_account_server(
            json!({
                "code": "0",
                "msg": "",
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let error = client
            .get_max_avail_size("BTC-USDT", "cash")
            .await
            .expect_err("empty OKX max-avail-size data must not become an empty success object");

        assert!(error.to_string().contains("缺少 data[0]"));

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn account_config_rejects_success_payload_without_config_item() {
        let (base_url, stop_server) = start_mock_account_server(
            json!({
                "code": "0",
                "msg": "",
                "data": []
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let error = client
            .get_account_config()
            .await
            .expect_err("empty OKX account config data must not become an empty success object");

        assert!(error.to_string().contains("缺少 data[0]"));

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn set_leverage_rejects_non_numeric_lever_before_transport() {
        let error = test_client()
            .set_leverage("BTC-USDT-SWAP", "bad-lever", "cross", "")
            .await
            .expect_err("non-numeric lever must be rejected before HTTP submit");

        assert!(error.to_string().contains("lever"));
    }

    #[tokio::test]
    async fn set_leverage_rejects_invalid_margin_mode_before_transport() {
        let error = test_client()
            .set_leverage("BTC-USDT-SWAP", "5", "portfolio", "")
            .await
            .expect_err("invalid mgnMode must be rejected before HTTP submit");

        assert!(error.to_string().contains("mgnMode"));
    }

    #[tokio::test]
    async fn set_leverage_rejects_invalid_position_side_before_transport() {
        let error = test_client()
            .set_leverage("BTC-USDT-SWAP", "5", "isolated", "flat")
            .await
            .expect_err("invalid posSide must be rejected before HTTP submit");

        assert!(error.to_string().contains("posSide"));
    }

    #[tokio::test]
    async fn set_position_mode_rejects_invalid_mode_before_transport() {
        let error = test_client()
            .set_position_mode("hedge")
            .await
            .expect_err("invalid posMode must be rejected before HTTP submit");

        assert!(error.to_string().contains("posMode"));
    }

    async fn start_mock_account_server(body: String) -> (String, oneshot::Sender<()>) {
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
