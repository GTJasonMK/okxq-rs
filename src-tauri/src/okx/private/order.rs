use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    okx::normalized_okx_client_order_id,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OkxAttachedAlgoOrder {
    pub attach_algo_client_order_id: Option<String>,
    pub tp_trigger_px: Option<String>,
    pub tp_ord_px: Option<String>,
    pub tp_trigger_px_type: Option<String>,
    pub sl_trigger_px: Option<String>,
    pub sl_ord_px: Option<String>,
    pub sl_trigger_px_type: Option<String>,
}

impl OkxAttachedAlgoOrder {
    pub fn stop_loss_market(trigger_px: impl Into<String>) -> Self {
        Self {
            sl_trigger_px: Some(trigger_px.into()),
            sl_ord_px: Some("-1".to_string()),
            sl_trigger_px_type: Some("last".to_string()),
            ..Self::default()
        }
    }

    pub fn take_profit_market(trigger_px: impl Into<String>) -> Self {
        Self {
            tp_trigger_px: Some(trigger_px.into()),
            tp_ord_px: Some("-1".to_string()),
            tp_trigger_px_type: Some("last".to_string()),
            ..Self::default()
        }
    }

    fn to_value(&self) -> AppResult<Option<Value>> {
        let mut item = serde_json::Map::new();
        insert_non_empty(&mut item, "tpTriggerPx", self.tp_trigger_px.as_deref());
        insert_non_empty(&mut item, "tpOrdPx", self.tp_ord_px.as_deref());
        insert_non_empty(
            &mut item,
            "tpTriggerPxType",
            self.tp_trigger_px_type.as_deref(),
        );
        insert_non_empty(&mut item, "slTriggerPx", self.sl_trigger_px.as_deref());
        insert_non_empty(&mut item, "slOrdPx", self.sl_ord_px.as_deref());
        insert_non_empty(
            &mut item,
            "slTriggerPxType",
            self.sl_trigger_px_type.as_deref(),
        );
        if item.is_empty() {
            return Ok(None);
        }
        if let Some(client_order_id) = normalized_okx_client_order_id(
            self.attach_algo_client_order_id.as_deref().unwrap_or(""),
        )? {
            item.insert("attachAlgoClOrdId".to_string(), json!(client_order_id));
        }
        Ok((!item.is_empty()).then_some(Value::Object(item)))
    }
}

pub(super) fn build_place_order_body(
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
    let inst_id = inst_id.trim().to_uppercase();
    let td_mode = td_mode.trim().to_lowercase();
    let ord_type = normalized_order_type(ord_type)?;
    let side = normalized_order_side(side)?;
    let sz = positive_decimal_text(sz, "sz")?;
    let pos_side = normalized_pos_side(pos_side)?;
    if inst_id.is_empty() {
        return Err(AppError::Validation(
            "OKX 下单参数 instId 不能为空".to_string(),
        ));
    }
    if td_mode.is_empty() {
        return Err(AppError::Validation(
            "OKX 下单参数 tdMode 不能为空".to_string(),
        ));
    }
    if reduce_only && is_cash_spot_order(&inst_id, &td_mode) {
        return Err(AppError::Validation(
            "OKX cash 现货下单不支持 reduceOnly".to_string(),
        ));
    }
    let px = px.trim();
    let price = if order_type_requires_price(ord_type) || (ord_type != "market" && !px.is_empty()) {
        Some(positive_decimal_text(px, "px")?)
    } else {
        None
    };
    let mut body = json!({
        "instId": inst_id,
        "tdMode": td_mode,
        "side": side,
        "ordType": ord_type,
        "sz": sz,
    });
    if let Some(price) = price {
        body["px"] = json!(price);
    }
    if ord_type == "market"
        && is_spot_inst_id(body.get("instId").and_then(Value::as_str).unwrap_or(""))
    {
        body["tgtCcy"] = json!("base_ccy");
    }
    if let Some(pos_side) = pos_side {
        body["posSide"] = json!(pos_side);
    }
    if reduce_only {
        body["reduceOnly"] = json!(true);
    }
    if let Some(client_order_id) = normalized_okx_client_order_id(client_order_id)? {
        body["clOrdId"] = json!(client_order_id);
    }
    let attached_algo_orders = attached_algo_orders.iter().try_fold(
        Vec::new(),
        |mut orders, order| -> AppResult<Vec<Value>> {
            if let Some(value) = order.to_value()? {
                orders.push(value);
            }
            Ok(orders)
        },
    )?;
    if !attached_algo_orders.is_empty() {
        body["attachAlgoOrds"] = Value::Array(attached_algo_orders);
    }
    Ok(body)
}

pub(super) fn build_place_algo_order_body(
    inst_id: &str,
    td_mode: &str,
    side: &str,
    sz: &str,
    pos_side: &str,
    reduce_only: bool,
    client_order_id: &str,
    algo_order: &OkxAttachedAlgoOrder,
) -> AppResult<Value> {
    let inst_id = inst_id.trim().to_uppercase();
    let td_mode = td_mode.trim().to_lowercase();
    let side = normalized_order_side(side)?;
    let sz = positive_decimal_text(sz, "sz")?;
    let pos_side = normalized_pos_side(pos_side)?;
    if inst_id.is_empty() {
        return Err(AppError::Validation(
            "OKX 策略委托参数 instId 不能为空".to_string(),
        ));
    }
    if td_mode.is_empty() {
        return Err(AppError::Validation(
            "OKX 策略委托参数 tdMode 不能为空".to_string(),
        ));
    }
    if reduce_only && is_cash_spot_order(&inst_id, &td_mode) {
        return Err(AppError::Validation(
            "OKX cash 现货策略委托不支持 reduceOnly".to_string(),
        ));
    }
    let tp_trigger_px = algo_order.tp_trigger_px.as_deref().map(str::trim);
    let sl_trigger_px = algo_order.sl_trigger_px.as_deref().map(str::trim);
    let has_take_profit = tp_trigger_px.is_some_and(|value| !value.is_empty());
    let has_stop_loss = sl_trigger_px.is_some_and(|value| !value.is_empty());
    match (has_take_profit, has_stop_loss) {
        (true, true) => {
            return Err(AppError::Validation(
                "OKX 独立保护单一次只支持一个止盈或止损触发条件".to_string(),
            ));
        }
        (false, false) => {
            return Err(AppError::Validation(
                "OKX 独立保护单缺少止盈或止损触发价".to_string(),
            ));
        }
        _ => {}
    }

    let mut body = json!({
        "instId": inst_id,
        "tdMode": td_mode,
        "side": side,
        "ordType": "conditional",
        "sz": sz,
    });
    if let Some(pos_side) = pos_side {
        body["posSide"] = json!(pos_side);
    }
    if reduce_only {
        body["reduceOnly"] = json!(true);
    }
    if let Some(client_order_id) = normalized_okx_client_order_id(client_order_id)? {
        body["algoClOrdId"] = json!(client_order_id);
    }
    if has_stop_loss {
        body["slTriggerPx"] = json!(positive_decimal_text(
            algo_order.sl_trigger_px.as_deref().unwrap_or_default(),
            "slTriggerPx",
        )?);
        body["slOrdPx"] = json!(algo_order_price_text(
            algo_order.sl_ord_px.as_deref().unwrap_or("-1"),
            "slOrdPx",
        )?);
        body["slTriggerPxType"] = json!(normalized_trigger_price_type(
            algo_order.sl_trigger_px_type.as_deref().unwrap_or("last"),
        )?);
    } else {
        body["tpTriggerPx"] = json!(positive_decimal_text(
            algo_order.tp_trigger_px.as_deref().unwrap_or_default(),
            "tpTriggerPx",
        )?);
        body["tpOrdPx"] = json!(algo_order_price_text(
            algo_order.tp_ord_px.as_deref().unwrap_or("-1"),
            "tpOrdPx",
        )?);
        body["tpTriggerPxType"] = json!(normalized_trigger_price_type(
            algo_order.tp_trigger_px_type.as_deref().unwrap_or("last"),
        )?);
    }
    Ok(body)
}

fn normalized_order_side(side: &str) -> AppResult<&'static str> {
    match side.trim().to_ascii_lowercase().as_str() {
        "buy" => Ok("buy"),
        "sell" => Ok("sell"),
        _ => Err(AppError::Validation(
            "OKX 下单参数 side 必须为 buy/sell".to_string(),
        )),
    }
}

pub(crate) fn normalized_order_type(ord_type: &str) -> AppResult<&'static str> {
    match ord_type.trim().to_ascii_lowercase().as_str() {
        "market" => Ok("market"),
        "limit" => Ok("limit"),
        "post_only" => Ok("post_only"),
        "fok" => Ok("fok"),
        "ioc" => Ok("ioc"),
        "optimal_limit_ioc" => Ok("optimal_limit_ioc"),
        "mmp" => Ok("mmp"),
        "mmp_and_post_only" => Ok("mmp_and_post_only"),
        "" => Err(AppError::Validation(
            "OKX 下单参数 ordType 不能为空".to_string(),
        )),
        _ => Err(AppError::Validation(
            "OKX 下单参数 ordType 不支持".to_string(),
        )),
    }
}

fn normalized_pos_side(pos_side: &str) -> AppResult<Option<&'static str>> {
    match pos_side.trim().to_ascii_lowercase().as_str() {
        "" => Ok(None),
        "long" => Ok(Some("long")),
        "short" => Ok(Some("short")),
        _ => Err(AppError::Validation(
            "OKX 下单参数 posSide 必须为空或为 long/short".to_string(),
        )),
    }
}

pub(crate) fn order_type_requires_price(ord_type: &str) -> bool {
    matches!(
        ord_type,
        "limit" | "post_only" | "fok" | "ioc" | "mmp" | "mmp_and_post_only"
    )
}

fn positive_decimal_text<'a>(value: &'a str, field: &str) -> AppResult<&'a str> {
    let trimmed = value.trim();
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| AppError::Validation(format!("OKX 下单参数 {field} 必须为正数")))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(trimmed)
    } else {
        Err(AppError::Validation(format!(
            "OKX 下单参数 {field} 必须为正数"
        )))
    }
}

fn algo_order_price_text<'a>(value: &'a str, field: &str) -> AppResult<&'a str> {
    let trimmed = value.trim();
    if trimmed == "-1" {
        return Ok(trimmed);
    }
    positive_decimal_text(trimmed, field)
}

fn normalized_trigger_price_type(value: &str) -> AppResult<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "last" => Ok("last"),
        "index" => Ok("index"),
        "mark" => Ok("mark"),
        _ => Err(AppError::Validation(
            "OKX 策略委托触发价类型必须为 last/index/mark".to_string(),
        )),
    }
}

fn insert_non_empty(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        map.insert(key.to_string(), json!(value));
    }
}

fn is_spot_inst_id(inst_id: &str) -> bool {
    !inst_id.trim().to_uppercase().ends_with("-SWAP")
}

fn is_cash_spot_order(inst_id: &str, td_mode: &str) -> bool {
    is_spot_inst_id(inst_id) && td_mode == "cash"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_order_body_does_not_send_tgt_ccy_for_swap_market_order() {
        let body = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "0.01",
            "",
            "long",
            false,
            "",
            &[],
        )
        .expect("valid swap market order body");

        assert_eq!(
            body.get("instId").and_then(Value::as_str),
            Some("BTC-USDT-SWAP")
        );
        assert_eq!(body.get("tdMode").and_then(Value::as_str), Some("cross"));
        assert_eq!(body.get("ordType").and_then(Value::as_str), Some("market"));
        assert!(body.get("tgtCcy").is_none());
        assert_eq!(body.get("posSide").and_then(Value::as_str), Some("long"));
    }

    #[test]
    fn place_order_body_keeps_tgt_ccy_for_spot_market_order() {
        let body = build_place_order_body(
            "BTC-USDT",
            "cash",
            "buy",
            "market",
            "0.01",
            "",
            "",
            false,
            "",
            &[],
        )
        .expect("valid spot market order body");

        assert_eq!(body.get("instId").and_then(Value::as_str), Some("BTC-USDT"));
        assert_eq!(body.get("tdMode").and_then(Value::as_str), Some("cash"));
        assert_eq!(body.get("tgtCcy").and_then(Value::as_str), Some("base_ccy"));
        assert!(body.get("posSide").is_none());
    }

    #[test]
    fn place_order_body_rejects_reduce_only_for_cash_spot_order() {
        let error = build_place_order_body(
            "BTC-USDT",
            "cash",
            "sell",
            "market",
            "0.01",
            "",
            "",
            true,
            "manualspotreduceonly",
            &[],
        )
        .expect_err("cash spot reduceOnly must not produce an OKX order body");

        assert!(error.to_string().contains("reduceOnly"));
    }

    #[test]
    fn place_order_body_attaches_stop_loss_algo_order() {
        let mut attached_order = OkxAttachedAlgoOrder::stop_loss_market("94000");
        attached_order.attach_algo_client_order_id = Some("attachedrisk1".to_string());
        let body = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "1",
            "",
            "long",
            false,
            "live1",
            &[attached_order],
        )
        .expect("valid attached stop-loss order body");

        let attached = body
            .get("attachAlgoOrds")
            .and_then(Value::as_array)
            .expect("attached algos should be present");
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0]["slTriggerPx"].as_str(), Some("94000"));
        assert_eq!(attached[0]["slOrdPx"].as_str(), Some("-1"));
        assert_eq!(attached[0]["slTriggerPxType"].as_str(), Some("last"));
        assert_eq!(
            attached[0]["attachAlgoClOrdId"].as_str(),
            Some("attachedrisk1")
        );
    }

    #[test]
    fn place_order_body_ignores_attached_algo_with_only_client_order_id() {
        let body = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "1",
            "",
            "long",
            false,
            "live1",
            &[OkxAttachedAlgoOrder {
                attach_algo_client_order_id: Some("attachedrisk1".to_string()),
                ..OkxAttachedAlgoOrder::default()
            }],
        )
        .expect("empty attached algo should not invalidate the parent order");

        assert!(body.get("attachAlgoOrds").is_none());
    }

    #[test]
    fn place_algo_order_body_builds_standalone_stop_loss_market() {
        let body = build_place_algo_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "sell",
            "2",
            "long",
            false,
            "riskalgo1",
            &OkxAttachedAlgoOrder::stop_loss_market("94000"),
        )
        .expect("valid standalone stop-loss algo order body");

        assert_eq!(
            body.get("instId").and_then(Value::as_str),
            Some("BTC-USDT-SWAP")
        );
        assert_eq!(body.get("tdMode").and_then(Value::as_str), Some("cross"));
        assert_eq!(body.get("side").and_then(Value::as_str), Some("sell"));
        assert_eq!(
            body.get("ordType").and_then(Value::as_str),
            Some("conditional")
        );
        assert_eq!(body.get("sz").and_then(Value::as_str), Some("2"));
        assert_eq!(body.get("posSide").and_then(Value::as_str), Some("long"));
        assert_eq!(
            body.get("algoClOrdId").and_then(Value::as_str),
            Some("riskalgo1")
        );
        assert_eq!(
            body.get("slTriggerPx").and_then(Value::as_str),
            Some("94000")
        );
        assert_eq!(body.get("slOrdPx").and_then(Value::as_str), Some("-1"));
        assert_eq!(
            body.get("slTriggerPxType").and_then(Value::as_str),
            Some("last")
        );
        assert!(body.get("tpTriggerPx").is_none());
    }

    #[test]
    fn place_algo_order_body_rejects_combined_stop_loss_and_take_profit() {
        let algo = OkxAttachedAlgoOrder {
            attach_algo_client_order_id: None,
            sl_trigger_px: Some("94000".to_string()),
            sl_ord_px: Some("-1".to_string()),
            sl_trigger_px_type: Some("last".to_string()),
            tp_trigger_px: Some("106000".to_string()),
            tp_ord_px: Some("-1".to_string()),
            tp_trigger_px_type: Some("last".to_string()),
        };

        let error = build_place_algo_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "sell",
            "2",
            "long",
            false,
            "riskalgo1",
            &algo,
        )
        .expect_err("standalone algo action must not become two protective orders");

        assert!(error.to_string().contains("一次只支持一个止盈或止损"));
    }

    #[test]
    fn place_order_body_keeps_price_for_post_only_order() {
        let body = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "post_only",
            "1",
            "65000",
            "long",
            false,
            "manual1",
            &[],
        )
        .expect("valid post-only order body");

        assert_eq!(
            body.get("ordType").and_then(Value::as_str),
            Some("post_only")
        );
        assert_eq!(body.get("px").and_then(Value::as_str), Some("65000"));
    }

    #[test]
    fn place_order_body_rejects_post_only_without_price_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "post_only",
            "1",
            "",
            "long",
            false,
            "manual1",
            &[],
        )
        .expect_err("post_only without px must not produce an OKX order body");

        assert!(error.to_string().contains("px"));
    }

    #[test]
    fn place_order_body_rejects_unknown_order_type_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "teleport",
            "1",
            "",
            "long",
            false,
            "manual1",
            &[],
        )
        .expect_err("unknown ordType must not produce an OKX order body");

        assert!(error.to_string().contains("ordType"));
    }

    #[test]
    fn place_order_body_rejects_invalid_pos_side_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "1",
            "",
            "flat",
            false,
            "manual1",
            &[],
        )
        .expect_err("posSide=flat must not produce an OKX order body");

        assert!(error.to_string().contains("posSide"));
    }

    #[test]
    fn place_order_body_rejects_invalid_side_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "hold",
            "market",
            "1",
            "",
            "long",
            false,
            "manual1",
            &[],
        )
        .expect_err("side=hold must not produce an OKX order body");

        assert!(error.to_string().contains("side"));
    }

    #[test]
    fn place_order_body_rejects_non_numeric_size_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "bad-size",
            "",
            "long",
            false,
            "manual1",
            &[],
        )
        .expect_err("non-numeric sz must not produce an OKX order body");

        assert!(error.to_string().contains("sz"));
    }

    #[test]
    fn place_order_body_rejects_invalid_client_order_id_before_submit() {
        let error = build_place_order_body(
            "BTC-USDT-SWAP",
            "cross",
            "buy",
            "market",
            "1",
            "",
            "long",
            false,
            "okxq_bad",
            &[],
        )
        .expect_err("invalid clOrdId must not be sent to OKX");

        assert!(error.to_string().contains("clOrdId"));
    }
}
