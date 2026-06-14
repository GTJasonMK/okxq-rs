use serde_json::{json, Value};

use super::helpers::{parse_value_f64, value_string};

pub(in crate::commands::local_api::trading) fn empty_account_balance() -> Value {
    json!({"total_equity": null, "total_eq": null, "iso_eq": null, "adj_eq": null, "usdt_balance": null, "usdt_available": null, "usdt_equity_usd": null, "details": []})
}

pub(in crate::commands::local_api::trading) fn normalize_account_balance(
    items: Vec<Value>,
) -> Value {
    let Some(account) = items.first() else {
        return empty_account_balance();
    };
    let raw_details = account
        .get("details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_total_equity = sum_finite(
        raw_details
            .iter()
            .filter_map(|item| first_value_f64(item, &["eqUsd", "disEq", "eq"])),
    );
    let total_equity = value_f64_opt(account, "totalEq").or(detail_total_equity);
    let iso_equity = value_f64_opt(account, "isoEq");
    let adjusted_equity = value_f64_opt(account, "adjEq");
    let usdt_detail = raw_details
        .iter()
        .find(|item| value_string(item, "ccy", "").eq_ignore_ascii_case("USDT"));
    let usdt_balance = usdt_detail.and_then(|item| {
        ["cashBal", "eq", "availBal", "availEq"]
            .into_iter()
            .find_map(|key| parse_value_f64(item.get(key)))
    });
    let usdt_available = usdt_detail.and_then(|item| {
        ["availBal", "availEq", "cashBal", "eq"]
            .into_iter()
            .find_map(|key| parse_value_f64(item.get(key)))
    });
    let usdt_equity_usd = usdt_detail.and_then(|item| first_value_f64(item, &["eqUsd", "disEq"]));
    let details = raw_details
        .into_iter()
        .map(|item| {
            json!({
                "ccy": value_string(&item, "ccy", ""),
                "avail_bal": value_f64_opt(&item, "availBal"),
                "avail_eq": value_f64_opt(&item, "availEq"),
                "frozen_bal": value_f64_opt(&item, "frozenBal"),
                "ord_frozen": value_f64_opt(&item, "ordFrozen"),
                "cash_bal": value_f64_opt(&item, "cashBal"),
                "eq": value_f64_opt(&item, "eq"),
                "eq_usd": value_f64_opt(&item, "eqUsd"),
                "dis_eq": value_f64_opt(&item, "disEq"),
                "u_time": value_string(&item, "uTime", ""),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "total_equity": total_equity,
        "total_eq": total_equity,
        "iso_eq": iso_equity,
        "adj_eq": adjusted_equity,
        "usdt_balance": usdt_balance,
        "usdt_available": usdt_available,
        "usdt_equity_usd": usdt_equity_usd,
        "details": details
    })
}

pub(in crate::commands::local_api::trading) fn normalize_holdings_from_balance(
    items: Vec<Value>,
) -> Vec<Value> {
    let Some(account) = items.first() else {
        return Vec::new();
    };
    account
        .get("details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let ccy = value_string(&item, "ccy", "");
            if ccy.is_empty() {
                return None;
            }
            let cash = parse_value_f64(item.get("cashBal"));
            let avail = parse_value_f64(item.get("availBal"));
            let frozen = first_value_f64(&item, &["frozenBal", "ordFrozen"]);
            let eq_usd = value_f64_opt(&item, "eqUsd");
            let dis_eq = value_f64_opt(&item, "disEq");
            let ord_frozen = value_f64_opt(&item, "ordFrozen");
            let total = cash
                .filter(|value| *value > 0.0)
                .or_else(|| sum_known([avail, frozen]));
            if ![total, avail, frozen, eq_usd, dis_eq, ord_frozen]
                .into_iter()
                .flatten()
                .any(|value| value > 0.0)
            {
                return None;
            }
            Some(json!({
                "ccy": ccy,
                "total": total,
                "available": avail,
                "frozen": frozen,
                "eq_usd": eq_usd,
                "dis_eq": dis_eq,
                "ord_frozen": ord_frozen,
                "avg_cost": Value::Null,
                "total_cost": Value::Null,
                "total_fee": Value::Null,
                "is_stablecoin": is_stablecoin(&ccy),
            }))
        })
        .collect()
}

fn first_value_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| parse_value_f64(value.get(*key)))
}

fn value_f64_opt(value: &Value, key: &str) -> Option<f64> {
    parse_value_f64(value.get(key))
}

fn sum_known(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    let mut found = false;
    let mut total = 0.0;
    for value in values.into_iter().flatten() {
        found = true;
        total += value;
    }
    found.then_some(total)
}

fn sum_finite(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut found = false;
    let mut total = 0.0;
    for value in values {
        found = true;
        total += value;
    }
    found.then_some(total)
}

fn is_stablecoin(ccy: &str) -> bool {
    matches!(
        ccy.trim().to_uppercase().as_str(),
        "USDT" | "USDC" | "USD" | "DAI" | "TUSD" | "FDUSD"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn account_balance_does_not_fabricate_zero_from_invalid_private_numbers() {
        let account = normalize_account_balance(vec![json!({
            "totalEq": "bad-total-equity",
            "isoEq": "bad-isolated-equity",
            "adjEq": "bad-adjusted-equity",
            "details": [{
                "ccy": "USDT",
                "cashBal": "bad-cash",
                "availBal": "bad-available",
                "availEq": "bad-available-equity",
                "frozenBal": "bad-frozen",
                "ordFrozen": "bad-order-frozen",
                "eq": "bad-equity",
                "eqUsd": "bad-equity-usd",
                "disEq": "bad-discount-equity",
                "uTime": "1780000000000"
            }]
        })]);

        assert!(account["total_eq"].is_null());
        assert!(account["total_equity"].is_null());
        assert!(account["iso_eq"].is_null());
        assert!(account["adj_eq"].is_null());
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
        assert!(account["details"][0]["cash_bal"].is_null());
        assert!(account["details"][0]["avail_bal"].is_null());
        assert!(account["details"][0]["eq_usd"].is_null());
    }

    #[test]
    fn account_balance_does_not_fabricate_zero_from_empty_private_payload() {
        let account = normalize_account_balance(Vec::new());

        assert!(account["total_eq"].is_null());
        assert!(account["total_equity"].is_null());
        assert!(account["iso_eq"].is_null());
        assert!(account["adj_eq"].is_null());
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
        assert_eq!(account["details"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn account_balance_does_not_fabricate_zero_usdt_when_usdt_detail_is_absent() {
        let account = normalize_account_balance(vec![json!({
            "totalEq": "70000",
            "details": [{
                "ccy": "BTC",
                "cashBal": "1",
                "eqUsd": "70000",
                "uTime": "1780000000000"
            }]
        })]);

        assert_eq!(account["total_eq"], 70000.0);
        assert_eq!(account["details"].as_array().map(Vec::len), Some(1));
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
    }

    #[test]
    fn account_balance_keeps_valid_private_number_strings() {
        let account = normalize_account_balance(vec![json!({
            "totalEq": "1234.5",
            "isoEq": "1200",
            "adjEq": "1210",
            "details": [{
                "ccy": "USDT",
                "cashBal": "1000",
                "availBal": "950",
                "availEq": "960",
                "eqUsd": "1001.5",
                "uTime": "1780000000000"
            }]
        })]);

        assert_eq!(account["total_eq"], 1234.5);
        assert_eq!(account["total_equity"], 1234.5);
        assert_eq!(account["iso_eq"], 1200.0);
        assert_eq!(account["adj_eq"], 1210.0);
        assert_eq!(account["usdt_balance"], 1000.0);
        assert_eq!(account["usdt_available"], 950.0);
        assert_eq!(account["usdt_equity_usd"], 1001.5);
        assert_eq!(account["details"][0]["cash_bal"], 1000.0);
        assert_eq!(account["details"][0]["avail_bal"], 950.0);
        assert_eq!(account["details"][0]["eq_usd"], 1001.5);
    }

    #[test]
    fn holdings_do_not_fabricate_zero_or_drop_asset_from_invalid_amounts() {
        let holdings = normalize_holdings_from_balance(vec![json!({
            "details": [{
                "ccy": "BTC",
                "cashBal": "bad-cash",
                "availBal": "bad-available",
                "frozenBal": "bad-frozen",
                "ordFrozen": "bad-order-frozen",
                "eqUsd": "70000",
                "disEq": "69900"
            }]
        })]);

        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0]["ccy"], "BTC");
        assert!(holdings[0]["total"].is_null());
        assert!(holdings[0]["available"].is_null());
        assert!(holdings[0]["frozen"].is_null());
        assert!(holdings[0]["ord_frozen"].is_null());
        assert_eq!(holdings[0]["eq_usd"], 70000.0);
        assert_eq!(holdings[0]["dis_eq"], 69900.0);
    }
}
