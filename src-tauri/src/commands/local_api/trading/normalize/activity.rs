use serde_json::{json, Value};

use super::helpers::{parse_value_f64, positive_i64_opt, value_string};

pub(in crate::commands::local_api::trading) fn empty_max_size() -> Value {
    json!({"max_buy": 0.0, "max_sell": 0.0, "data": []})
}

pub(in crate::commands::local_api::trading) fn normalize_positions(
    items: Vec<Value>,
) -> Vec<Value> {
    items
        .into_iter()
        .map(|item| {
            json!({
                "inst_id": value_string(&item, "instId", ""),
                "inst_type": value_string(&item, "instType", ""),
                "pos_side": normalized_position_side(&item),
                "pos": value_f64_opt(&item, "pos"),
                "avg_px": value_f64_opt(&item, "avgPx"),
                "mark_px": value_f64_opt(&item, "markPx"),
                "upl": value_f64_opt(&item, "upl"),
                "upl_ratio": value_f64_opt(&item, "uplRatio"),
                "lever": value_f64_opt(&item, "lever"),
                "liq_px": value_f64_opt(&item, "liqPx"),
                "margin": value_f64_opt(&item, "margin").or_else(|| value_f64_opt(&item, "imr")),
                "mgn_mode": value_string(&item, "mgnMode", ""),
                "c_time": positive_i64_opt(&item, "cTime"),
                "u_time": positive_i64_opt(&item, "uTime"),
                "raw": item,
            })
        })
        .collect()
}

pub(in crate::commands::local_api::trading) fn normalize_orders(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .map(|item| {
            json!({
                "ord_id": value_string(&item, "ordId", ""),
                "cl_ord_id": value_string(&item, "clOrdId", ""),
                "inst_id": value_string(&item, "instId", ""),
                "inst_type": value_string(&item, "instType", ""),
                "side": normalized_order_side(&item),
                "ord_type": value_string(&item, "ordType", ""),
                "px": value_f64_opt(&item, "px"),
                "sz": value_f64_opt(&item, "sz"),
                "fill_sz": value_f64_opt(&item, "fillSz"),
                "fill_px": value_f64_opt(&item, "fillPx"),
                "avg_px": value_f64_opt(&item, "avgPx"),
                "pnl": value_f64_opt(&item, "pnl"),
                "state": value_string(&item, "state", ""),
                "c_time": positive_i64_opt(&item, "cTime"),
                "u_time": positive_i64_opt(&item, "uTime"),
                "raw": item,
            })
        })
        .collect()
}

pub(in crate::commands::local_api::trading) fn normalize_fills(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .map(|item| {
            json!({
                "trade_id": value_string(&item, "tradeId", ""),
                "ord_id": value_string(&item, "ordId", ""),
                "inst_id": value_string(&item, "instId", ""),
                "inst_type": value_string(&item, "instType", ""),
                "side": normalized_order_side(&item),
                "fill_px": value_f64_opt(&item, "fillPx"),
                "fill_sz": value_f64_opt(&item, "fillSz"),
                "fee": value_f64_opt(&item, "fee"),
                "fee_ccy": value_string(&item, "feeCcy", ""),
                "ts": positive_i64_opt(&item, "ts"),
                "source": "okx_private_rest",
                "raw": item,
            })
        })
        .collect()
}

pub(in crate::commands::local_api::trading) fn normalize_max_size(items: Vec<Value>) -> Value {
    let Some(item) = items.first() else {
        return empty_max_size();
    };
    json!({
        "max_buy": value_f64_opt(item, "maxBuy"),
        "max_sell": value_f64_opt(item, "maxSell"),
        "data": items
    })
}

fn normalized_order_side(item: &Value) -> &'static str {
    match value_string(item, "side", "")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "buy" => "buy",
        "sell" => "sell",
        _ => "",
    }
}

fn normalized_position_side(item: &Value) -> &'static str {
    match value_string(item, "posSide", "")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "long" => "long",
        "short" => "short",
        _ => match value_f64_opt(item, "pos") {
            Some(pos) if pos < 0.0 => "short",
            Some(pos) if pos > 0.0 => "long",
            Some(_) => "",
            None => "",
        },
    }
}

fn value_f64_opt(value: &Value, key: &str) -> Option<f64> {
    parse_value_f64(value.get(key))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn trading_activity_normalizers_do_not_fabricate_zero_from_invalid_numbers() {
        let positions = normalize_positions(vec![json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "pos": "bad-position",
            "avgPx": "bad-entry",
            "markPx": "bad-mark",
            "upl": "bad-upl",
            "uplRatio": "bad-upl-ratio",
            "lever": "bad-leverage",
            "liqPx": "bad-liq",
            "imr": "bad-margin",
            "cTime": "bad-create-time",
            "uTime": "bad-update-time"
        })]);
        let orders = normalize_orders(vec![json!({
            "ordId": "order-invalid",
            "instId": "BTC-USDT-SWAP",
            "side": "buy",
            "px": "bad-price",
            "sz": "bad-size",
            "fillSz": "bad-fill-size",
            "fillPx": "bad-fill-price",
            "avgPx": "bad-average",
            "pnl": "bad-pnl",
            "cTime": "bad-create-time",
            "uTime": "bad-update-time"
        })]);
        let fills = normalize_fills(vec![json!({
            "tradeId": "fill-invalid",
            "instId": "BTC-USDT-SWAP",
            "side": "sell",
            "fillPx": "bad-fill-price",
            "fillSz": "bad-fill-size",
            "fee": "bad-fee",
            "ts": "bad-fill-time"
        })]);

        assert!(positions[0]["pos"].is_null());
        assert_eq!(positions[0]["pos_side"], "");
        assert!(positions[0]["avg_px"].is_null());
        assert!(positions[0]["mark_px"].is_null());
        assert!(positions[0]["upl"].is_null());
        assert!(positions[0]["upl_ratio"].is_null());
        assert!(positions[0]["lever"].is_null());
        assert!(positions[0]["margin"].is_null());
        assert!(positions[0]["c_time"].is_null());
        assert!(positions[0]["u_time"].is_null());
        assert!(orders[0]["px"].is_null());
        assert!(orders[0]["sz"].is_null());
        assert!(orders[0]["fill_sz"].is_null());
        assert!(orders[0]["fill_px"].is_null());
        assert!(orders[0]["avg_px"].is_null());
        assert!(orders[0]["pnl"].is_null());
        assert!(orders[0]["c_time"].is_null());
        assert!(orders[0]["u_time"].is_null());
        assert!(fills[0]["fill_px"].is_null());
        assert!(fills[0]["fill_sz"].is_null());
        assert!(fills[0]["fee"].is_null());
        assert!(fills[0]["ts"].is_null());
    }

    #[test]
    fn trading_activity_normalizers_keep_valid_number_strings() {
        let positions = normalize_positions(vec![json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "pos": "-2",
            "avgPx": "100",
            "markPx": "95",
            "upl": "10",
            "uplRatio": "0.05",
            "lever": "3",
            "liqPx": "80",
            "imr": "50"
        })]);
        let orders = normalize_orders(vec![json!({
            "ordId": "order-valid",
            "instId": "BTC-USDT-SWAP",
            "side": "buy",
            "px": "100",
            "sz": "2",
            "fillSz": "1",
            "fillPx": "99",
            "avgPx": "99.5",
            "pnl": "3.5"
        })]);
        let fills = normalize_fills(vec![json!({
            "tradeId": "fill-valid",
            "instId": "BTC-USDT-SWAP",
            "side": "sell",
            "fillPx": "101",
            "fillSz": "2",
            "fee": "-0.2"
        })]);

        assert_eq!(positions[0]["pos"], -2.0);
        assert_eq!(positions[0]["pos_side"], "short");
        assert_eq!(positions[0]["avg_px"], 100.0);
        assert_eq!(positions[0]["mark_px"], 95.0);
        assert_eq!(positions[0]["upl"], 10.0);
        assert_eq!(positions[0]["upl_ratio"], 0.05);
        assert_eq!(positions[0]["lever"], 3.0);
        assert_eq!(positions[0]["margin"], 50.0);
        assert_eq!(orders[0]["px"], 100.0);
        assert_eq!(orders[0]["sz"], 2.0);
        assert_eq!(orders[0]["fill_sz"], 1.0);
        assert_eq!(orders[0]["fill_px"], 99.0);
        assert_eq!(orders[0]["avg_px"], 99.5);
        assert_eq!(orders[0]["pnl"], 3.5);
        assert_eq!(fills[0]["fill_px"], 101.0);
        assert_eq!(fills[0]["fill_sz"], 2.0);
        assert_eq!(fills[0]["fee"], -0.2);
    }

    #[test]
    fn zero_net_position_does_not_infer_long_side() {
        let positions = normalize_positions(vec![json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "posSide": "net",
            "pos": "0",
        })]);

        assert_eq!(positions[0]["pos"], 0.0);
        assert_eq!(positions[0]["pos_side"], "");
    }
}
