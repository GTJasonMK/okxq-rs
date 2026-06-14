use anyhow::{anyhow, Result};
use serde_json::{Map, Value};

use crate::instrument::infer_spot_swap_inst_type;

use super::types::PriceAlert;

pub(in crate::alerts) fn alert_from_map(obj: &Map<String, Value>) -> Result<PriceAlert> {
    let now = now_text();
    let inst_id = normalize_inst_id(value_string(obj, "inst_id").as_deref().unwrap_or(""))?;
    let inst_type = infer_inst_type(
        &inst_id,
        value_string(obj, "inst_type").as_deref().unwrap_or(""),
    );
    let symbol = value_string(obj, "symbol")
        .map(|value| normalize_symbol_label(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_symbol_label(&inst_id));

    let alert = PriceAlert {
        id: format!("pa_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]),
        inst_id,
        symbol,
        inst_type,
        alert_type: value_string(obj, "alert_type")
            .unwrap_or_else(|| "price".to_string())
            .trim()
            .to_lowercase(),
        direction: value_string(obj, "direction")
            .unwrap_or_else(|| "above".to_string())
            .trim()
            .to_lowercase(),
        target_price: optional_f64(obj, "target_price")?,
        change_percent: optional_f64(obj, "change_percent")?,
        note: value_string(obj, "note")
            .unwrap_or_default()
            .trim()
            .to_string(),
        enabled: value_bool(obj, "enabled", true),
        trigger_once: value_bool(obj, "trigger_once", true),
        cooldown_seconds: value_i64(obj, "cooldown_seconds", 300).max(0),
        created_at: value_string(obj, "created_at").unwrap_or_else(|| now.clone()),
        updated_at: value_string(obj, "updated_at").unwrap_or_else(|| now.clone()),
        triggered_at: value_string(obj, "triggered_at"),
        last_value: optional_f64(obj, "last_value")?,
        last_trigger_value: optional_f64(obj, "last_trigger_value")?,
        last_trigger_ts: value_i64(obj, "last_trigger_ts", 0).max(0),
    };
    validate_alert(&alert)?;
    Ok(alert)
}

pub(in crate::alerts) fn validate_alert(alert: &PriceAlert) -> Result<()> {
    if !matches!(alert.alert_type.as_str(), "price" | "change") {
        return Err(anyhow!("alert_type 仅支持 price 或 change"));
    }
    if !matches!(alert.direction.as_str(), "above" | "below") {
        return Err(anyhow!("direction 仅支持 above 或 below"));
    }
    if alert.alert_type == "price" {
        let target_price = alert.target_price.unwrap_or(0.0);
        if !target_price.is_finite() || target_price <= 0.0 {
            return Err(anyhow!("价格提醒必须提供大于 0 的 target_price"));
        }
    } else if alert.change_percent.is_none() {
        return Err(anyhow!("涨跌幅提醒必须提供 change_percent"));
    }
    Ok(())
}

pub(in crate::alerts) fn normalize_inst_id(value: &str) -> Result<String> {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        return Err(anyhow!("inst_id 不能为空"));
    }
    Ok(normalized)
}

pub(in crate::alerts) fn normalize_symbol_label(value: &str) -> String {
    value.trim().to_uppercase().replace("-SWAP", "")
}

pub(in crate::alerts) fn infer_inst_type(inst_id: &str, inst_type: &str) -> String {
    let value = inst_type.trim().to_uppercase();
    if !value.is_empty() {
        return value;
    }
    infer_spot_swap_inst_type(inst_id).to_string()
}

pub(in crate::alerts) fn value_string(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        _ => None,
    })
}

pub(in crate::alerts) fn value_bool(obj: &Map<String, Value>, key: &str, default: bool) -> bool {
    obj.get(key)
        .and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            _ => None,
        })
        .unwrap_or(default)
}

pub(in crate::alerts) fn value_i64(obj: &Map<String, Value>, key: &str, default: i64) -> i64 {
    obj.get(key).and_then(Value::as_i64).unwrap_or(default)
}

pub(in crate::alerts) fn optional_f64(obj: &Map<String, Value>, key: &str) -> Result<Option<f64>> {
    let Some(value) = obj.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let number = value.as_f64().ok_or_else(|| anyhow!("{key} 必须是数字"))?;
    if !number.is_finite() {
        return Err(anyhow!("{key} 必须是有效数字"));
    }
    Ok(Some(number))
}

pub(in crate::alerts) fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn alert_payload_accepts_typed_json_values() {
        let payload = json!({
            "inst_id": "btc-usdt-swap",
            "inst_type": "SWAP",
            "alert_type": "price",
            "direction": "above",
            "target_price": 100.5,
            "enabled": false,
            "trigger_once": true,
            "cooldown_seconds": 120,
            "last_trigger_ts": 1_700_000_000_000_i64
        });
        let alert = alert_from_map(payload.as_object().expect("object payload"))
            .expect("typed alert payload should parse");

        assert_eq!(alert.inst_id, "BTC-USDT-SWAP");
        assert_eq!(alert.inst_type, "SWAP");
        assert_eq!(alert.target_price, Some(100.5));
        assert!(!alert.enabled);
        assert!(alert.trigger_once);
        assert_eq!(alert.cooldown_seconds, 120);
        assert_eq!(alert.last_trigger_ts, 1_700_000_000_000);
    }

    #[test]
    fn alert_payload_rejects_string_numbers() {
        let payload = json!({
            "inst_id": "btc-usdt-swap",
            "alert_type": "price",
            "direction": "above",
            "target_price": "100.5",
            "cooldown_seconds": "120",
            "enabled": "false"
        });

        assert!(alert_from_map(payload.as_object().expect("object payload")).is_err());
    }
}
