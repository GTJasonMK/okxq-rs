use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    okx::normalize::payload_data,
    okx::normalized_okx_client_order_id,
};

use super::{
    order::{build_place_algo_order_body, build_place_order_body, OkxAttachedAlgoOrder},
    params::{
        normalized_required_inst_id, optional_inst_filter_params, push_optional_uppercase_param,
    },
    OkxPrivateClient,
};

impl OkxPrivateClient {
    pub async fn get_pending_orders(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
    ) -> AppResult<Vec<Value>> {
        let params = optional_inst_filter_params(inst_type, inst_id);
        let payload = self
            .get_json(
                "trade.orders_pending",
                "/api/v5/trade/orders-pending",
                &params,
                inst_id,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_order_history(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![("limit", limit.clamp(1, 100).to_string())];
        push_optional_uppercase_param(&mut params, "instType", inst_type);
        push_optional_uppercase_param(&mut params, "instId", inst_id);
        let payload = self
            .get_json(
                "trade.orders_history",
                "/api/v5/trade/orders-history",
                &params,
                inst_id,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_fills(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![("limit", limit.clamp(1, 100).to_string())];
        push_optional_uppercase_param(&mut params, "instType", inst_type);
        push_optional_uppercase_param(&mut params, "instId", inst_id);
        let payload = self
            .get_json("trade.fills", "/api/v5/trade/fills", &params, inst_id)
            .await?;
        Ok(payload_data(payload))
    }

    /// 下单（现货/合约通用）
    /// - `pos_side`: 合约持仓方向，"long"/"short"（现货不传）
    /// - `reduce_only`: 仅减仓（合约用）
    pub async fn place_order(
        &self,
        inst_id: &str,
        td_mode: &str,
        side: &str,
        ord_type: &str,
        sz: &str,
        px: &str,
        pos_side: &str,
        reduce_only: bool,
        client_order_id: &str,
    ) -> AppResult<Value> {
        self.place_order_with_attached_algos(
            inst_id,
            td_mode,
            side,
            ord_type,
            sz,
            px,
            pos_side,
            reduce_only,
            client_order_id,
            &[],
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn place_order_with_attached_algos(
        &self,
        inst_id: &str,
        td_mode: &str,
        side: &str,
        ord_type: &str,
        sz: &str,
        px: &str,
        pos_side: &str,
        reduce_only: bool,
        client_order_id: &str,
        attached_algo_orders: &[OkxAttachedAlgoOrder],
    ) -> AppResult<Value> {
        let body = build_place_order_body(
            inst_id,
            td_mode,
            side,
            ord_type,
            sz,
            px,
            pos_side,
            reduce_only,
            client_order_id,
            attached_algo_orders,
        )?;
        let payload = self
            .post_json(
                "trade.place_order",
                "/api/v5/trade/order",
                &body,
                Some(inst_id),
            )
            .await?;
        first_order_operation_result(payload, "下单")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn place_algo_order(
        &self,
        inst_id: &str,
        td_mode: &str,
        side: &str,
        sz: &str,
        pos_side: &str,
        reduce_only: bool,
        client_order_id: &str,
        algo_order: &OkxAttachedAlgoOrder,
    ) -> AppResult<Value> {
        let body = build_place_algo_order_body(
            inst_id,
            td_mode,
            side,
            sz,
            pos_side,
            reduce_only,
            client_order_id,
            algo_order,
        )?;
        let payload = self
            .post_json(
                "trade.place_algo_order",
                "/api/v5/trade/order-algo",
                &body,
                Some(inst_id),
            )
            .await?;
        first_order_operation_result(payload, "策略委托")
    }

    pub async fn get_conditional_algo_orders_pending(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
        algo_id: &str,
        limit: u32,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![("ordType", "conditional".to_string())];
        if !algo_id.trim().is_empty() {
            params.push(("algoId", algo_id.trim().to_string()));
        }
        push_optional_uppercase_param(&mut params, "instType", inst_type);
        push_optional_uppercase_param(&mut params, "instId", inst_id);
        params.push(("limit", limit.clamp(1, 100).to_string()));
        let payload = self
            .get_json(
                "trade.orders_algo_pending",
                "/api/v5/trade/orders-algo-pending",
                &params,
                inst_id,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn get_conditional_algo_orders_history(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
        state: Option<&str>,
        algo_id: &str,
        limit: u32,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![("ordType", "conditional".to_string())];
        if let Some(state) = state.map(str::trim).filter(|value| !value.is_empty()) {
            params.push(("state", state.to_ascii_lowercase()));
        }
        if !algo_id.trim().is_empty() {
            params.push(("algoId", algo_id.trim().to_string()));
        }
        if !params.iter().any(|(key, _)| *key == "state")
            && !params.iter().any(|(key, _)| *key == "algoId")
        {
            return Err(AppError::Validation(
                "OKX 策略委托历史查询参数 state/algoId 至少传一个".to_string(),
            ));
        }
        push_optional_uppercase_param(&mut params, "instType", inst_type);
        push_optional_uppercase_param(&mut params, "instId", inst_id);
        params.push(("limit", limit.clamp(1, 100).to_string()));
        let payload = self
            .get_json(
                "trade.orders_algo_history",
                "/api/v5/trade/orders-algo-history",
                &params,
                inst_id,
            )
            .await?;
        Ok(payload_data(payload))
    }

    pub async fn cancel_algo_order(
        &self,
        inst_id: &str,
        algo_id: &str,
        algo_client_order_id: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "撤销策略委托")?;
        let (algo_id, algo_client_order_id) =
            normalized_required_algo_identity(algo_id, algo_client_order_id, "撤销策略委托")?;
        let mut body = json!([{ "instId": inst_id.as_str() }]);
        if let Some(algo_id) = algo_id {
            body[0]["algoId"] = json!(algo_id);
        }
        if let Some(algo_client_order_id) = algo_client_order_id {
            body[0]["algoClOrdId"] = json!(algo_client_order_id);
        }
        let payload = self
            .post_json(
                "trade.cancel_algo_order",
                "/api/v5/trade/cancel-algos",
                &body,
                Some(&inst_id),
            )
            .await?;
        first_order_operation_result(payload, "撤销策略委托")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn amend_algo_order(
        &self,
        inst_id: &str,
        algo_id: &str,
        algo_client_order_id: &str,
        new_sz: &str,
        new_trigger_px: &str,
        amend_take_profit: bool,
        cancel_on_fail: bool,
        request_id: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "修改策略委托")?;
        let (algo_id, algo_client_order_id) =
            normalized_required_algo_identity(algo_id, algo_client_order_id, "修改策略委托")?;
        let new_sz = normalized_optional_positive_decimal(new_sz, "newSz", "修改策略委托")?;
        let new_trigger_px =
            normalized_optional_positive_decimal(new_trigger_px, "newTriggerPx", "修改策略委托")?;
        if new_sz.is_none() && new_trigger_px.is_none() {
            return Err(AppError::Validation(
                "OKX 修改策略委托参数 newSz/newTriggerPx 至少传一个".to_string(),
            ));
        }
        let request_id = request_id.trim();
        let mut body = json!({
            "instId": inst_id.as_str(),
            "cxlOnFail": cancel_on_fail,
        });
        if let Some(algo_id) = algo_id {
            body["algoId"] = json!(algo_id);
        }
        if let Some(algo_client_order_id) = algo_client_order_id {
            body["algoClOrdId"] = json!(algo_client_order_id);
        }
        if let Some(new_sz) = new_sz {
            body["newSz"] = json!(new_sz);
        }
        if let Some(new_trigger_px) = new_trigger_px {
            if amend_take_profit {
                body["newTpTriggerPx"] = json!(new_trigger_px);
                body["newTpOrdPx"] = json!("-1");
                body["newTpTriggerPxType"] = json!("last");
            } else {
                body["newSlTriggerPx"] = json!(new_trigger_px);
                body["newSlOrdPx"] = json!("-1");
                body["newSlTriggerPxType"] = json!("last");
            }
        }
        if !request_id.is_empty() {
            body["reqId"] = json!(request_id);
        }
        let payload = self
            .post_json(
                "trade.amend_algo_order",
                "/api/v5/trade/amend-algos",
                &body,
                Some(&inst_id),
            )
            .await?;
        first_order_operation_result(payload, "修改策略委托")
    }

    /// 撤单
    /// - `ord_id` 和 `client_order_id` 至少传一个
    pub async fn cancel_order(
        &self,
        inst_id: &str,
        ord_id: &str,
        client_order_id: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "撤单")?;
        let (ord_id, client_order_id) =
            normalized_required_order_identity(ord_id, client_order_id, "撤单")?;
        let mut body = json!({ "instId": inst_id.as_str() });
        if let Some(ord_id) = ord_id {
            body["ordId"] = json!(ord_id);
        }
        if let Some(client_order_id) = client_order_id {
            body["clOrdId"] = json!(client_order_id);
        }
        let payload = self
            .post_json(
                "trade.cancel_order",
                "/api/v5/trade/cancel-order",
                &body,
                Some(&inst_id),
            )
            .await?;
        first_order_operation_result(payload, "撤单")
    }

    /// 改单
    /// - `ord_id` 和 `client_order_id` 至少传一个
    /// - `new_sz` 和 `new_px` 至少传一个
    pub async fn amend_order(
        &self,
        inst_id: &str,
        ord_id: &str,
        client_order_id: &str,
        new_sz: &str,
        new_px: &str,
        cancel_on_fail: bool,
        request_id: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "改单")?;
        let (ord_id, client_order_id) =
            normalized_required_order_identity(ord_id, client_order_id, "改单")?;
        let new_sz = normalized_optional_positive_decimal(new_sz, "newSz", "改单")?;
        let new_px = normalized_optional_positive_decimal(new_px, "newPx", "改单")?;
        if new_sz.is_none() && new_px.is_none() {
            return Err(AppError::Validation(
                "OKX 改单参数 newSz/newPx 至少传一个".to_string(),
            ));
        }
        let request_id = request_id.trim();
        let mut body = json!({
            "instId": inst_id.as_str(),
            "cxlOnFail": cancel_on_fail,
        });
        if let Some(ord_id) = ord_id {
            body["ordId"] = json!(ord_id);
        }
        if let Some(client_order_id) = client_order_id {
            body["clOrdId"] = json!(client_order_id);
        }
        if let Some(new_sz) = new_sz {
            body["newSz"] = json!(new_sz);
        }
        if let Some(new_px) = new_px {
            body["newPx"] = json!(new_px);
        }
        if !request_id.is_empty() {
            body["reqId"] = json!(request_id);
        }
        let payload = self
            .post_json(
                "trade.amend_order",
                "/api/v5/trade/amend-order",
                &body,
                Some(&inst_id),
            )
            .await?;
        first_order_operation_result(payload, "改单")
    }

    /// 查询单个订单详情
    pub async fn get_order(
        &self,
        inst_id: &str,
        ord_id: &str,
        client_order_id: &str,
    ) -> AppResult<Value> {
        let inst_id = normalized_required_inst_id(inst_id, "查单")?;
        let (ord_id, client_order_id) =
            normalized_required_order_identity(ord_id, client_order_id, "查单")?;
        let mut params = vec![("instId", inst_id.clone())];
        if let Some(ord_id) = ord_id {
            params.push(("ordId", ord_id));
        }
        if let Some(client_order_id) = client_order_id {
            params.push(("clOrdId", client_order_id));
        }
        let payload = self
            .get_json(
                "trade.order_detail",
                "/api/v5/trade/order",
                &params,
                Some(&inst_id),
            )
            .await?;
        first_order_operation_result(payload, "查单")
    }

    /// 获取历史成交记录（最近 3 个月）
    pub async fn get_fills_history(
        &self,
        inst_type: &str,
        inst_id: &str,
        limit: u32,
        after: &str,
        before: &str,
    ) -> AppResult<Vec<Value>> {
        let mut params = vec![
            ("instType", inst_type.trim().to_uppercase()),
            ("limit", limit.clamp(1, 100).to_string()),
        ];
        if !inst_id.is_empty() {
            params.push(("instId", inst_id.trim().to_uppercase()));
        }
        if !after.is_empty() {
            params.push(("after", after.to_string()));
        }
        if !before.is_empty() {
            params.push(("before", before.to_string()));
        }
        let payload = self
            .get_json(
                "trade.fills_history",
                "/api/v5/trade/fills-history",
                &params,
                if inst_id.is_empty() {
                    None
                } else {
                    Some(inst_id)
                },
            )
            .await?;
        Ok(payload_data(payload))
    }
}

fn normalized_required_order_identity(
    ord_id: &str,
    client_order_id: &str,
    operation: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    let ord_id = ord_id.trim();
    let client_order_id = normalized_okx_client_order_id(client_order_id)?;
    if ord_id.is_empty() && client_order_id.is_none() {
        return Err(AppError::Validation(format!(
            "OKX {operation}参数 ordId/clOrdId 至少传一个"
        )));
    }
    Ok((
        (!ord_id.is_empty()).then(|| ord_id.to_string()),
        client_order_id.map(str::to_string),
    ))
}

fn normalized_required_algo_identity(
    algo_id: &str,
    algo_client_order_id: &str,
    operation: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    let algo_id = algo_id.trim();
    let algo_client_order_id = normalized_okx_client_order_id(algo_client_order_id)?;
    if algo_id.is_empty() && algo_client_order_id.is_none() {
        return Err(AppError::Validation(format!(
            "OKX {operation}参数 algoId/algoClOrdId 至少传一个"
        )));
    }
    Ok((
        (!algo_id.is_empty()).then(|| algo_id.to_string()),
        algo_client_order_id.map(str::to_string),
    ))
}

fn normalized_optional_positive_decimal(
    value: &str,
    field: &str,
    operation: &str,
) -> AppResult<Option<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| AppError::Validation(format!("OKX {operation}参数 {field} 必须是有限正数")))?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(AppError::Validation(format!(
            "OKX {operation}参数 {field} 必须是有限正数"
        )));
    }
    Ok(Some(trim_decimal_text(value)))
}

fn trim_decimal_text(value: &str) -> String {
    let value = value.trim();
    if !value.contains('.') {
        return value.to_string();
    }
    value
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn first_order_operation_result(payload: Value, operation: &str) -> AppResult<Value> {
    payload_data(payload)
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Runtime(format!("OKX {operation}响应缺少 data[0] 订单结果")))
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
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

    #[tokio::test]
    async fn cancel_order_rejects_blank_order_identity_before_transport() {
        let error = test_client()
            .cancel_order("BTC-USDT", " ", "")
            .await
            .expect_err("blank ordId/clOrdId must be rejected before HTTP submit");

        assert!(error.to_string().contains("ordId/clOrdId"));
    }

    #[tokio::test]
    async fn cancel_order_rejects_invalid_client_order_id_before_transport() {
        let error = test_client()
            .cancel_order("BTC-USDT", "", "okxq_bad")
            .await
            .expect_err("invalid clOrdId must be rejected before HTTP submit");

        assert!(error.to_string().contains("clOrdId"));
    }

    #[tokio::test]
    async fn get_order_rejects_blank_order_identity_before_transport() {
        let error = test_client()
            .get_order("BTC-USDT", "", " ")
            .await
            .expect_err("blank ordId/clOrdId must be rejected before HTTP request");

        assert!(error.to_string().contains("ordId/clOrdId"));
    }

    #[tokio::test]
    async fn get_order_rejects_success_payload_without_order_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
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
            .get_order("BTC-USDT-SWAP", "missing-order", "")
            .await
            .expect_err("empty OKX order detail data must not become an empty successful order");

        assert!(error.to_string().contains("缺少 data[0]"));

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn place_order_returns_okx_order_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "ordId": "submitted-1",
                    "clOrdId": "local1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .place_order(
                "BTC-USDT-SWAP",
                "cross",
                "buy",
                "market",
                "1",
                "",
                "long",
                false,
                "local1",
            )
            .await
            .expect("OKX order submit should succeed");

        assert_eq!(
            response.get("ordId").and_then(Value::as_str),
            Some("submitted-1")
        );
        assert_eq!(
            response.get("clOrdId").and_then(Value::as_str),
            Some("local1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn place_algo_order_returns_okx_algo_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "algoId": "algo-1",
                    "algoClOrdId": "riskalgo1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .place_algo_order(
                "BTC-USDT-SWAP",
                "cross",
                "sell",
                "1",
                "long",
                false,
                "riskalgo1",
                &super::OkxAttachedAlgoOrder::stop_loss_market("94000"),
            )
            .await
            .expect("OKX algo order submit should succeed");

        assert_eq!(
            response.get("algoId").and_then(Value::as_str),
            Some("algo-1")
        );
        assert_eq!(
            response.get("algoClOrdId").and_then(Value::as_str),
            Some("riskalgo1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn get_conditional_algo_orders_pending_returns_payload_data() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "algoId": "algo-1",
                    "algoClOrdId": "riskalgo1",
                    "state": "live",
                    "ordType": "conditional"
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .get_conditional_algo_orders_pending(Some("SWAP"), Some("BTC-USDT-SWAP"), "algo-1", 10)
            .await
            .expect("pending algo orders should return payload data");

        assert_eq!(response.len(), 1);
        assert_eq!(
            response[0].get("algoId").and_then(Value::as_str),
            Some("algo-1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn get_conditional_algo_orders_history_requires_state_or_algo_id() {
        let error = test_client()
            .get_conditional_algo_orders_history(Some("SWAP"), Some("BTC-USDT-SWAP"), None, "", 10)
            .await
            .expect_err("history query without state/algoId must not submit HTTP request");

        assert!(error.to_string().contains("state/algoId"));
    }

    #[tokio::test]
    async fn cancel_algo_order_returns_okx_algo_cancel_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "algoId": "algo-1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .cancel_algo_order("BTC-USDT-SWAP", "algo-1", "")
            .await
            .expect("OKX algo cancel should succeed");

        assert_eq!(
            response.get("algoId").and_then(Value::as_str),
            Some("algo-1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn cancel_algo_order_rejects_blank_algo_id_before_transport() {
        let error = test_client()
            .cancel_algo_order("BTC-USDT-SWAP", " ", "")
            .await
            .expect_err("blank algoId/algoClOrdId must be rejected before HTTP submit");

        assert!(error.to_string().contains("algoId/algoClOrdId"));
    }

    #[tokio::test]
    async fn cancel_algo_order_accepts_client_order_id_before_transport() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "algoId": "",
                    "algoClOrdId": "riskalgo1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .cancel_algo_order("BTC-USDT-SWAP", "", "riskalgo1")
            .await
            .expect("OKX algo cancel by client order id should succeed");

        assert_eq!(
            response.get("algoClOrdId").and_then(Value::as_str),
            Some("riskalgo1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn amend_algo_order_rejects_blank_identity_before_transport() {
        let error = test_client()
            .amend_algo_order("BTC-USDT-SWAP", " ", "", "1", "", false, false, "")
            .await
            .expect_err("blank algo identity must be rejected before HTTP submit");

        assert!(error.to_string().contains("algoId/algoClOrdId"));
    }

    #[tokio::test]
    async fn amend_algo_order_rejects_blank_amendment_before_transport() {
        let error = test_client()
            .amend_algo_order("BTC-USDT-SWAP", "algo-1", "", "", " ", false, false, "")
            .await
            .expect_err("blank algo newSz/newTriggerPx must be rejected before HTTP submit");

        assert!(error.to_string().contains("newSz/newTriggerPx"));
    }

    #[tokio::test]
    async fn amend_algo_order_returns_okx_algo_amend_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "algoId": "algo-1",
                    "algoClOrdId": "riskalgo1",
                    "reqId": "algo-amend-1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .amend_algo_order(
                "BTC-USDT-SWAP",
                "algo-1",
                "riskalgo1",
                "2.0000",
                "94.5000",
                false,
                true,
                "algo-amend-1",
            )
            .await
            .expect("OKX algo amend should succeed");

        assert_eq!(
            response.get("algoId").and_then(Value::as_str),
            Some("algo-1")
        );
        assert_eq!(
            response.get("algoClOrdId").and_then(Value::as_str),
            Some("riskalgo1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn cancel_order_returns_okx_cancel_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "ordId": "cancelled-1",
                    "clOrdId": "localcancel1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .cancel_order("BTC-USDT-SWAP", "", "localcancel1")
            .await
            .expect("OKX cancel should succeed");

        assert_eq!(
            response.get("ordId").and_then(Value::as_str),
            Some("cancelled-1")
        );
        assert_eq!(
            response.get("clOrdId").and_then(Value::as_str),
            Some("localcancel1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn amend_order_rejects_blank_amendment_before_transport() {
        let error = test_client()
            .amend_order("BTC-USDT-SWAP", "order-1", "", "", " ", false, "")
            .await
            .expect_err("blank newSz/newPx must be rejected before HTTP submit");

        assert!(error.to_string().contains("newSz/newPx"));
    }

    #[tokio::test]
    async fn amend_order_returns_okx_amend_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
            json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "ordId": "amended-1",
                    "clOrdId": "localamend1",
                    "reqId": "req-amend-1",
                    "sCode": "0",
                    "sMsg": ""
                }]
            })
            .to_string(),
        )
        .await;
        let client = test_client_with_base_url(base_url);

        let response = client
            .amend_order(
                "BTC-USDT-SWAP",
                "",
                "localamend1",
                "2.0000",
                "101.5000",
                false,
                "req-amend-1",
            )
            .await
            .expect("OKX amend should succeed");

        assert_eq!(
            response.get("ordId").and_then(Value::as_str),
            Some("amended-1")
        );
        assert_eq!(
            response.get("clOrdId").and_then(Value::as_str),
            Some("localamend1")
        );

        let _ = stop_server.send(());
    }

    #[tokio::test]
    async fn place_order_rejects_success_payload_without_order_result_item() {
        let (base_url, stop_server) = start_mock_trade_server(
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
            .place_order(
                "BTC-USDT-SWAP",
                "cross",
                "buy",
                "market",
                "1",
                "",
                "long",
                false,
                "local1",
            )
            .await
            .expect_err("empty OKX order data must not become an empty successful order");

        assert!(error.to_string().contains("缺少 data[0]"));

        let _ = stop_server.send(());
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

    async fn start_mock_trade_server(body: String) -> (String, oneshot::Sender<()>) {
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
