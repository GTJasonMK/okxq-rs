use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use hmac::Mac;
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    okx_network::{
        OKX_BUSINESS_WS_URL, OKX_BUSINESS_WS_URL_SIMULATED, OKX_PRIVATE_WS_URL_LIVE,
        OKX_PRIVATE_WS_URL_SIMULATED,
    },
};

use super::{super::HmacSha256, values::*};

pub(in crate::realtime) fn normalize_private_mode(mode: &str) -> AppResult<String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "live" => Ok("live".to_string()),
        "simulated" => Ok("simulated".to_string()),
        other => Err(AppError::Validation(format!(
            "OKX 私有实时模式只支持 live 或 simulated，收到 {other}"
        ))),
    }
}

pub(in crate::realtime) fn private_ws_url(mode: &str) -> AppResult<&'static str> {
    match normalize_private_mode(mode)?.as_str() {
        "live" => Ok(OKX_PRIVATE_WS_URL_LIVE),
        _ => Ok(OKX_PRIVATE_WS_URL_SIMULATED),
    }
}

pub(in crate::realtime) fn private_business_ws_url(mode: &str) -> AppResult<&'static str> {
    match normalize_private_mode(mode)?.as_str() {
        "live" => Ok(OKX_BUSINESS_WS_URL),
        _ => Ok(OKX_BUSINESS_WS_URL_SIMULATED),
    }
}

pub(in crate::realtime) fn sign_private_ws_login(
    secret_key: &str,
    timestamp: &str,
) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .map_err(|error| anyhow!("OKX private websocket signing key invalid: {error}"))?;
    mac.update(format!("{timestamp}GET/users/self/verify").as_bytes());
    Ok(general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
}

pub(in crate::realtime) fn normalize_private_account_detail(
    detail: &Value,
    account: &Value,
    mode: &str,
) -> Option<Value> {
    let ccy = value_string(detail, "ccy")?;
    if ccy.trim().is_empty() {
        return None;
    }
    let u_time = positive_time_string(value_string(detail, "uTime"))
        .or_else(|| positive_time_string(value_string(account, "uTime")));
    Some(json!({
        "mode": mode,
        "ccy": ccy,
        "cashBal": value_string(detail, "cashBal"),
        "availBal": value_string(detail, "availBal"),
        "availEq": value_string(detail, "availEq"),
        "frozenBal": value_string(detail, "frozenBal"),
        "ordFrozen": value_string(detail, "ordFrozen"),
        "eq": value_string(detail, "eq"),
        "eqUsd": value_string(detail, "eqUsd"),
        "disEq": value_string(detail, "disEq"),
        "uTime": u_time,
        "raw": detail,
    }))
}

pub(in crate::realtime) fn normalize_private_account_summary(account: &Value, mode: &str) -> Value {
    json!({
        "mode": mode,
        "total_eq": parse_f64(account.get("totalEq")),
        "total_equity": parse_f64(account.get("totalEq")),
        "iso_eq": parse_f64(account.get("isoEq")),
        "adj_eq": parse_f64(account.get("adjEq")),
        "u_time": positive_i64(account.get("uTime")),
        "raw": account,
    })
}

pub(in crate::realtime) fn normalize_private_order(order: Value, mode: &str) -> Option<Value> {
    let ord_id = value_string(&order, "ordId").unwrap_or_default();
    let cl_ord_id = value_string(&order, "clOrdId").unwrap_or_default();
    if ord_id.trim().is_empty() && cl_ord_id.trim().is_empty() {
        return None;
    }
    let inst_id = value_string(&order, "instId").unwrap_or_default();
    let inst_type = required_inst_type(&order)?;
    let c_time = positive_i64(order.get("cTime"))?;
    let u_time = positive_i64(order.get("uTime"))?;
    Some(json!({
        "mode": mode,
        "ord_id": ord_id,
        "cl_ord_id": cl_ord_id,
        "inst_id": inst_id,
        "inst_type": inst_type,
        "side": value_string(&order, "side").unwrap_or_default(),
        "ord_type": value_string(&order, "ordType").unwrap_or_default(),
        "px": parse_f64(order.get("px")),
        "sz": parse_f64(order.get("sz")),
        "fill_sz": parse_f64(order.get("fillSz").or_else(|| order.get("accFillSz"))),
        "fill_px": parse_f64(order.get("fillPx")),
        "avg_px": parse_f64(order.get("avgPx")),
        "pnl": parse_f64(order.get("pnl")),
        "state": value_string(&order, "state").unwrap_or_default(),
        "c_time": c_time,
        "u_time": u_time,
        "source": "okx_private_ws",
        "raw": order,
    }))
}

pub(in crate::realtime) fn normalize_private_algo_order(order: Value, mode: &str) -> Option<Value> {
    let algo_id = value_string(&order, "algoId").unwrap_or_default();
    let algo_cl_ord_id = value_string(&order, "algoClOrdId").unwrap_or_default();
    if algo_id.trim().is_empty() && algo_cl_ord_id.trim().is_empty() {
        return None;
    }
    let inst_id = value_string(&order, "instId").unwrap_or_default();
    let inst_type = required_inst_type(&order)?;
    let c_time = positive_i64(order.get("cTime"));
    let u_time = positive_i64(order.get("uTime"));
    Some(json!({
        "mode": mode,
        "algo_id": algo_id,
        "algo_cl_ord_id": algo_cl_ord_id,
        "actual_order_id": value_string(&order, "actualOrdId").or_else(|| value_string(&order, "ordId")).unwrap_or_default(),
        "actual_client_order_id": value_string(&order, "actualClOrdId").or_else(|| value_string(&order, "clOrdId")).unwrap_or_default(),
        "inst_id": inst_id,
        "inst_type": inst_type,
        "ord_type": value_string(&order, "ordType").unwrap_or_default(),
        "side": value_string(&order, "side").unwrap_or_default(),
        "state": value_string(&order, "state").unwrap_or_default(),
        "actual_px": parse_f64(order.get("actualPx")),
        "actual_sz": parse_f64(order.get("actualSz")),
        "actual_side": value_string(&order, "actualSide").unwrap_or_default(),
        "fail_code": value_string(&order, "failCode").unwrap_or_default(),
        "fail_reason": value_string(&order, "failReason").unwrap_or_default(),
        "c_time": c_time,
        "u_time": u_time,
        "source": "okx_business_private_ws",
        "raw": order,
    }))
}

pub(in crate::realtime) fn normalize_private_fill(fill: Value, mode: &str) -> Option<Value> {
    let trade_id = value_string(&fill, "tradeId")
        .or_else(|| value_string(&fill, "fillId"))
        .filter(|value| !value.is_empty())?;
    let inst_id = value_string(&fill, "instId").unwrap_or_default();
    let inst_type = required_inst_type(&fill)?;
    let fill_px = positive_f64(fill.get("fillPx").or_else(|| fill.get("px")))?;
    let fill_sz = positive_f64(fill.get("fillSz").or_else(|| fill.get("sz")))?;
    Some(json!({
        "mode": mode,
        "trade_id": trade_id,
        "ord_id": value_string(&fill, "ordId").unwrap_or_default(),
        "cl_ord_id": value_string(&fill, "clOrdId").unwrap_or_default(),
        "inst_id": inst_id,
        "inst_type": inst_type,
        "side": value_string(&fill, "side").unwrap_or_default(),
        "fill_px": fill_px,
        "fill_sz": fill_sz,
        "fee": parse_f64(fill.get("fee")),
        "fee_ccy": value_string(&fill, "feeCcy").unwrap_or_default(),
        "ts": positive_i64(fill.get("ts"))?,
        "source": "okx_private_ws",
        "raw": fill,
    }))
}

pub(in crate::realtime) fn normalize_private_position(
    position: Value,
    mode: &str,
) -> Option<Value> {
    let inst_id = value_string(&position, "instId")
        .filter(|value| !value.trim().is_empty())?
        .trim()
        .to_uppercase();
    let inst_type = required_inst_type(&position)?;
    let pos = parse_f64(position.get("pos"));
    let pos_side = normalized_position_side(&position, pos);
    let margin = parse_f64(position.get("margin")).or_else(|| parse_f64(position.get("imr")));
    let u_time = positive_i64(position.get("uTime"))?;

    Some(json!({
        "mode": mode,
        "inst_id": inst_id,
        "inst_type": inst_type,
        "pos_side": pos_side,
        "pos": pos,
        "avg_px": parse_f64(position.get("avgPx")),
        "mark_px": parse_f64(position.get("markPx")),
        "upl": parse_f64(position.get("upl")),
        "upl_ratio": parse_f64(position.get("uplRatio")),
        "lever": parse_f64(position.get("lever")),
        "liq_px": parse_f64(position.get("liqPx")),
        "margin": margin,
        "mgn_mode": value_string(&position, "mgnMode").unwrap_or_default(),
        "c_time": positive_i64(position.get("cTime")),
        "u_time": u_time,
        "source": "okx_private_ws",
        "raw": position,
    }))
}

fn required_inst_type(value: &Value) -> Option<String> {
    value_string(value, "instType")
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty())
}

fn normalized_position_side(position: &Value, pos: Option<f64>) -> &'static str {
    match value_string(position, "posSide")
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .as_str()
    {
        "long" => "long",
        "short" => "short",
        _ if pos.is_some_and(|value| value < 0.0) => "short",
        _ if pos.is_some_and(|value| value > 0.0) => "long",
        _ => "",
    }
}

fn positive_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = parse_f64(value)?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}

fn positive_i64(value: Option<&Value>) -> Option<i64> {
    value_i64(value).filter(|item| *item > 0)
}

fn positive_time_string(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    value
        .parse::<i64>()
        .ok()
        .filter(|item| *item > 0)
        .map(|_| value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        normalize_private_account_detail, normalize_private_account_summary,
        normalize_private_algo_order, normalize_private_fill, normalize_private_mode,
        normalize_private_order, normalize_private_position, private_business_ws_url,
        private_ws_url,
    };

    #[test]
    fn private_realtime_mode_rejects_removed_aliases() {
        assert_eq!(normalize_private_mode("live").unwrap(), "live");
        assert_eq!(normalize_private_mode("simulated").unwrap(), "simulated");
        assert_eq!(
            private_ws_url("live").unwrap(),
            crate::okx_network::OKX_PRIVATE_WS_URL_LIVE
        );
        assert_eq!(
            private_business_ws_url("simulated").unwrap(),
            crate::okx_network::OKX_BUSINESS_WS_URL_SIMULATED
        );
        for mode in ["paper", "demo", "simulation", "real", ""] {
            assert!(normalize_private_mode(mode).is_err());
            assert!(private_ws_url(mode).is_err());
            assert!(private_business_ws_url(mode).is_err());
        }
    }

    #[test]
    fn normalize_private_position_maps_mark_price_and_upl() {
        let position = normalize_private_position(
            json!({
                "instId": "btc-usdt-swap",
                "instType": "SWAP",
                "posSide": "short",
                "pos": "-2",
                "avgPx": "70000.5",
                "markPx": "71000.25",
                "upl": "-1999.5",
                "uplRatio": "-0.0285",
                "lever": "5",
                "liqPx": "76000",
                "imr": "100",
                "mgnMode": "cross",
                "cTime": "1710000000000",
                "uTime": "1710000002000"
            }),
            "simulated",
        )
        .expect("position should normalize");

        assert_eq!(position["mode"], "simulated");
        assert_eq!(position["inst_id"], "BTC-USDT-SWAP");
        assert_eq!(position["inst_type"], "SWAP");
        assert_eq!(position["pos_side"], "short");
        assert_eq!(position["pos"], -2.0);
        assert_eq!(position["avg_px"], 70000.5);
        assert_eq!(position["mark_px"], 71000.25);
        assert_eq!(position["upl"], -1999.5);
        assert_eq!(position["upl_ratio"], -0.0285);
        assert_eq!(position["margin"], 100.0);
        assert_eq!(position["mgn_mode"], "cross");
        assert_eq!(position["c_time"], 1710000000000_i64);
        assert_eq!(position["u_time"], 1710000002000_i64);
    }

    #[test]
    fn normalize_private_account_summary_does_not_fabricate_zero_from_invalid_numbers() {
        let account = normalize_private_account_summary(
            &json!({
                "totalEq": "bad-total-equity",
                "isoEq": "bad-isolated-equity",
                "adjEq": "bad-adjusted-equity",
                "uTime": "1780000000000",
            }),
            "live",
        );

        assert_eq!(account["mode"], "live");
        assert!(account["total_eq"].is_null());
        assert!(account["total_equity"].is_null());
        assert!(account["iso_eq"].is_null());
        assert!(account["adj_eq"].is_null());
        assert_eq!(account["u_time"], 1780000000000_i64);
    }

    #[test]
    fn normalize_private_fill_preserves_client_order_id_for_local_attribution() {
        let fill = normalize_private_fill(
            json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "tradeId": "trade-1",
                "ordId": "order-1",
                "clOrdId": "client-1",
                "side": "buy",
                "fillPx": "100.5",
                "fillSz": "1",
                "fee": "-0.01",
                "feeCcy": "USDT",
                "ts": "1780000000000"
            }),
            "simulated",
        )
        .expect("fill should normalize");

        assert_eq!(fill["trade_id"], "trade-1");
        assert_eq!(fill["ord_id"], "order-1");
        assert_eq!(fill["cl_ord_id"], "client-1");
        assert_eq!(fill["source"], "okx_private_ws");
    }

    #[test]
    fn normalize_private_order_does_not_copy_client_order_id_into_order_id() {
        let order = normalize_private_order(
            json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "clOrdId": "client-only",
                "side": "buy",
                "ordType": "market",
                "sz": "1",
                "state": "live",
                "cTime": "1780000000000",
                "uTime": "1780000000001"
            }),
            "simulated",
        )
        .expect("order should normalize with client id only");

        assert_eq!(order["ord_id"], "");
        assert_eq!(order["cl_ord_id"], "client-only");
    }

    #[test]
    fn normalize_private_algo_order_keeps_algo_identity_and_state() {
        let order = normalize_private_algo_order(
            json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "algoId": "algo-1",
                "algoClOrdId": "algo-client-1",
                "ordType": "conditional",
                "side": "sell",
                "state": "live",
                "actualSz": "",
                "failCode": "",
                "uTime": "1780000000001"
            }),
            "simulated",
        )
        .expect("algo order should normalize");

        assert_eq!(order["algo_id"], "algo-1");
        assert_eq!(order["algo_cl_ord_id"], "algo-client-1");
        assert_eq!(order["state"], "live");
        assert_eq!(order["source"], "okx_business_private_ws");
    }

    #[test]
    fn normalize_private_stream_items_require_inst_type_instead_of_inferring_from_inst_id() {
        assert!(normalize_private_order(
            json!({
                "instId": "BTC-USDT-SWAP",
                "ordId": "order-no-inst-type",
                "side": "buy",
                "ordType": "limit",
                "state": "live",
                "cTime": "1780000000000",
                "uTime": "1780000000001",
            }),
            "live",
        )
        .is_none());
        assert!(normalize_private_algo_order(
            json!({
                "instId": "BTC-USDT-SWAP",
                "algoId": "algo-no-inst-type",
                "ordType": "conditional",
                "state": "live",
            }),
            "live",
        )
        .is_none());
        assert!(normalize_private_fill(
            json!({
                "tradeId": "fill-no-inst-type",
                "instId": "BTC-USDT-SWAP",
                "fillPx": "100",
                "fillSz": "1",
                "ts": "1780000000000",
            }),
            "live",
        )
        .is_none());
        assert!(normalize_private_position(
            json!({
                "instId": "BTC-USDT-SWAP",
                "posSide": "long",
                "pos": "1",
                "uTime": "1780000000000",
            }),
            "live",
        )
        .is_none());
    }

    #[test]
    fn normalize_private_account_detail_does_not_fabricate_zero_for_missing_balances() {
        let detail = normalize_private_account_detail(
            &json!({
                "ccy": "USDT",
            }),
            &json!({
                "uTime": "1780000000000",
            }),
            "live",
        )
        .expect("currency should keep detail");

        assert_eq!(detail["mode"], "live");
        assert_eq!(detail["ccy"], "USDT");
        assert!(detail["cashBal"].is_null());
        assert!(detail["availBal"].is_null());
        assert!(detail["availEq"].is_null());
        assert!(detail["frozenBal"].is_null());
        assert!(detail["ordFrozen"].is_null());
        assert!(detail["eq"].is_null());
        assert!(detail["eqUsd"].is_null());
        assert!(detail["disEq"].is_null());
        assert_eq!(detail["uTime"], "1780000000000");
    }

    #[test]
    fn normalize_private_account_summary_does_not_use_receive_time_for_invalid_update_time() {
        let account = normalize_private_account_summary(
            &json!({
                "totalEq": "100",
                "uTime": "bad-time",
            }),
            "live",
        );

        assert_eq!(account["mode"], "live");
        assert_eq!(account["total_eq"], 100.0);
        assert!(account["u_time"].is_null());
    }

    #[test]
    fn normalize_private_account_detail_does_not_use_receive_time_for_invalid_update_time() {
        let detail = normalize_private_account_detail(
            &json!({
                "ccy": "USDT",
                "uTime": "bad-time",
            }),
            &json!({
                "uTime": "also-bad",
            }),
            "live",
        )
        .expect("currency should keep detail");

        assert_eq!(detail["mode"], "live");
        assert_eq!(detail["ccy"], "USDT");
        assert!(detail["uTime"].is_null());
    }

    #[test]
    fn normalize_private_position_infers_short_from_negative_net_position() {
        let position = normalize_private_position(
            json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "net",
                "pos": "-0.5",
                "markPx": "3000",
                "uTime": "1780000000000"
            }),
            "live",
        )
        .expect("position should normalize");

        assert_eq!(position["mode"], "live");
        assert_eq!(position["inst_id"], "ETH-USDT-SWAP");
        assert_eq!(position["pos_side"], "short");
        assert_eq!(position["pos"], -0.5);
        assert_eq!(position["mark_px"], 3000.0);
    }

    #[test]
    fn normalize_private_position_does_not_infer_long_from_zero_net_position() {
        let position = normalize_private_position(
            json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "net",
                "pos": "0",
                "markPx": "3000",
                "uTime": "1780000000000"
            }),
            "live",
        )
        .expect("position should normalize");

        assert_eq!(position["pos"], 0.0);
        assert_eq!(position["pos_side"], "");
    }

    #[test]
    fn normalize_private_position_does_not_fabricate_zero_from_invalid_numbers() {
        let position = normalize_private_position(
            json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "long",
                "pos": "bad-position",
                "avgPx": "bad-entry",
                "markPx": "bad-mark",
                "upl": "bad-upl",
                "uplRatio": "bad-upl-ratio",
                "lever": "bad-leverage",
                "liqPx": "bad-liq",
                "imr": "bad-margin",
                "uTime": "1780000000000"
            }),
            "live",
        )
        .expect("position should normalize");

        assert_eq!(position["pos_side"], "long");
        assert!(position["pos"].is_null());
        assert!(position["avg_px"].is_null());
        assert!(position["mark_px"].is_null());
        assert!(position["upl"].is_null());
        assert!(position["upl_ratio"].is_null());
        assert!(position["lever"].is_null());
        assert!(position["liq_px"].is_null());
        assert!(position["margin"].is_null());
        assert!(position["c_time"].is_null());
    }

    #[test]
    fn normalize_private_position_rejects_invalid_update_time_instead_of_using_receive_time() {
        assert!(normalize_private_position(
            json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "long",
                "pos": "1",
                "markPx": "3000",
                "uTime": "bad-time",
            }),
            "live",
        )
        .is_none());
    }

    #[test]
    fn normalize_private_fill_rejects_invalid_price_or_size() {
        assert!(normalize_private_fill(
            json!({
                "tradeId": "fill-bad-price",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "bad-price",
                "fillSz": "1",
                "side": "buy",
                "ts": "1780000000000",
            }),
            "live",
        )
        .is_none());

        assert!(normalize_private_fill(
            json!({
                "tradeId": "fill-bad-size",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "100",
                "fillSz": "bad-size",
                "side": "sell",
                "ts": "1780000000000",
            }),
            "live",
        )
        .is_none());

        assert!(normalize_private_fill(
            json!({
                "tradeId": "fill-zero-size",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "100",
                "fillSz": "0",
                "side": "sell",
                "ts": "1780000000000",
            }),
            "live",
        )
        .is_none());
    }

    #[test]
    fn normalize_private_fill_rejects_invalid_timestamp_instead_of_using_receive_time() {
        assert!(normalize_private_fill(
            json!({
                "tradeId": "fill-bad-ts",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "100",
                "fillSz": "1",
                "side": "buy",
                "ts": "bad-ts",
            }),
            "live",
        )
        .is_none());
    }

    #[test]
    fn normalize_private_fill_keeps_missing_or_invalid_fee_unknown() {
        let missing_fee = normalize_private_fill(
            json!({
                "tradeId": "fill-missing-fee",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "100",
                "fillSz": "1",
                "side": "buy",
                "ts": "1780000000000",
            }),
            "live",
        )
        .expect("fill with required execution evidence should normalize");

        let invalid_fee = normalize_private_fill(
            json!({
                "tradeId": "fill-invalid-fee",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "fillPx": "100",
                "fillSz": "1",
                "fee": "bad-fee",
                "side": "sell",
                "ts": "1780000000001",
            }),
            "live",
        )
        .expect("fill with invalid optional fee should keep execution evidence");

        assert!(missing_fee["fee"].is_null());
        assert!(invalid_fee["fee"].is_null());
    }

    #[test]
    fn normalize_private_order_does_not_fabricate_zero_from_invalid_numbers() {
        let order = normalize_private_order(
            json!({
                "ordId": "order-bad-numbers",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "side": "buy",
                "ordType": "limit",
                "state": "live",
                "sz": "bad-size",
                "px": "bad-price",
                "fillSz": "bad-fill-size",
                "fillPx": "bad-fill-price",
                "avgPx": "bad-average-price",
                "pnl": "bad-pnl",
                "cTime": "1780000000000",
                "uTime": "1780000000001",
            }),
            "live",
        )
        .expect("order id should keep event");

        assert_eq!(order["mode"], "live");
        assert_eq!(order["ord_id"], "order-bad-numbers");
        assert!(order["sz"].is_null());
        assert!(order["px"].is_null());
        assert!(order["fill_sz"].is_null());
        assert!(order["fill_px"].is_null());
        assert!(order["avg_px"].is_null());
        assert!(order["pnl"].is_null());
    }

    #[test]
    fn normalize_private_order_rejects_invalid_create_time_instead_of_fabricating_epoch() {
        assert!(normalize_private_order(
            json!({
                "ordId": "order-bad-create-time",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "side": "buy",
                "ordType": "limit",
                "state": "live",
                "sz": "1",
                "px": "100",
                "cTime": "bad-time",
                "uTime": "1780000000001",
            }),
            "live",
        )
        .is_none());
    }

    #[test]
    fn normalize_private_order_rejects_invalid_update_time_instead_of_fabricating_epoch() {
        assert!(normalize_private_order(
            json!({
                "ordId": "order-bad-update-time",
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "side": "buy",
                "ordType": "limit",
                "state": "live",
                "sz": "1",
                "px": "100",
                "cTime": "1780000000000",
                "uTime": "bad-time",
            }),
            "live",
        )
        .is_none());
    }
}
