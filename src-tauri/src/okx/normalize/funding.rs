use serde_json::{json, Value};

use super::values::{parse_f64, parse_i64, value_string};

pub fn normalize_funding_rate(item: Value) -> Option<Value> {
    let inst_id = value_string(&item, "instId").filter(|value| !value.is_empty())?;
    let inst_type = value_string(&item, "instType").filter(|value| !value.is_empty())?;
    let method = value_string(&item, "method").filter(|value| !value.is_empty())?;
    let formula_type = value_string(&item, "formulaType").filter(|value| !value.is_empty())?;
    let funding_rate = parse_f64(item.get("fundingRate")?)?;
    let funding_time = parse_i64(item.get("fundingTime")?)?;
    if funding_time <= 0 {
        return None;
    }
    Some(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "funding_time": funding_time,
        "timestamp": funding_time,
        "funding_rate": funding_rate,
        "rate": funding_rate,
        "realized_rate": parse_f64(item.get("realizedRate").unwrap_or(&Value::Null)),
        "next_funding_time": parse_i64(item.get("nextFundingTime").unwrap_or(&Value::Null)),
        "min_funding_rate": parse_f64(item.get("minFundingRate").unwrap_or(&Value::Null)),
        "max_funding_rate": parse_f64(item.get("maxFundingRate").unwrap_or(&Value::Null)),
        "method": method,
        "formula_type": formula_type,
        "payload": item,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn funding_rate_rejects_invalid_funding_time_instead_of_fabricating_epoch() {
        let normalized = normalize_funding_rate(json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "method": "current_period",
            "formulaType": "withRate",
            "fundingRate": "0.0001",
            "fundingTime": "bad-time"
        }));

        assert!(normalized.is_none());
    }

    #[test]
    fn funding_rate_rejects_missing_inst_id_instead_of_using_request_scope() {
        let normalized = normalize_funding_rate(json!({
            "instType": "SWAP",
            "method": "current_period",
            "formulaType": "withRate",
            "fundingRate": "0.0001",
            "fundingTime": "1710000000000"
        }));

        assert!(normalized.is_none());
    }

    #[test]
    fn funding_rate_keeps_valid_funding_time_and_rate() {
        let normalized = normalize_funding_rate(json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "method": "current_period",
            "formulaType": "withRate",
            "fundingRate": "0.0001",
            "fundingTime": "1710000000000"
        }))
        .expect("valid funding payload");

        assert_eq!(normalized["funding_time"].as_i64(), Some(1_710_000_000_000));
        assert_eq!(normalized["timestamp"].as_i64(), Some(1_710_000_000_000));
        assert_eq!(normalized["funding_rate"].as_f64(), Some(0.0001));
        assert_eq!(normalized["inst_type"].as_str(), Some("SWAP"));
        assert_eq!(normalized["method"].as_str(), Some("current_period"));
        assert_eq!(normalized["formula_type"].as_str(), Some("withRate"));
    }
}
