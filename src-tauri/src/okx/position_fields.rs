use serde_json::Value;

pub(crate) fn okx_account_equity(items: &[Value]) -> Option<f64> {
    items.iter().find_map(|item| {
        okx_positive_value(item, "totalEq")
            .or_else(|| okx_positive_value(item, "adjEq"))
            .or_else(|| {
                let details = item.get("details")?.as_array()?;
                let mut total = 0.0;
                let mut found = false;
                for detail in details {
                    if let Some(value) = okx_positive_value(detail, "eqUsd")
                        .or_else(|| okx_positive_value(detail, "disEq"))
                        .or_else(|| okx_positive_value(detail, "eq"))
                    {
                        total += value;
                        found = true;
                    }
                }
                found.then_some(total)
            })
            .filter(|value| value.is_finite() && *value > 0.0)
    })
}

pub(crate) fn okx_signed_position(item: &Value, pos: f64) -> f64 {
    match okx_value_text(item, "posSide")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "short" => -pos.abs(),
        "long" => pos.abs(),
        _ => pos,
    }
}

pub(crate) fn okx_position_side_dir(item: &Value, pos: f64) -> i32 {
    match okx_value_text(item, "posSide")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "short" => -1,
        "long" => 1,
        _ if pos < 0.0 => -1,
        _ => 1,
    }
}

pub(crate) fn okx_position_side_label(item: &Value, position: f64) -> &'static str {
    match okx_value_text(item, "posSide")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "long" => "long",
        "short" => "short",
        _ if position > 0.0 => "long",
        _ if position < 0.0 => "short",
        _ => "",
    }
}

pub(crate) fn okx_position_notional(
    item: &Value,
    position_abs: f64,
    fallback_price_keys: &[&str],
) -> Option<f64> {
    okx_positive_abs_value(item, "notionalUsd")
        .or_else(|| okx_positive_abs_value(item, "notional"))
        .or_else(|| okx_positive_abs_value(item, "notionalUsdForBorrow"))
        .or_else(|| {
            fallback_price_keys
                .iter()
                .find_map(|key| okx_positive_value(item, key))
                .map(|price| position_abs * price)
        })
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(crate) fn okx_positive_value(value: &Value, key: &str) -> Option<f64> {
    okx_finite_value(value, key).filter(|item| *item > 0.0)
}

pub(crate) fn okx_finite_value(value: &Value, key: &str) -> Option<f64> {
    let parsed = match value.get(key)? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    parsed.is_finite().then_some(parsed)
}

pub(crate) fn okx_value_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(item)) => item.clone(),
        Some(Value::Number(item)) => item.to_string(),
        Some(Value::Bool(item)) => item.to_string(),
        _ => String::new(),
    }
}

fn okx_positive_abs_value(value: &Value, key: &str) -> Option<f64> {
    okx_finite_value(value, key)
        .map(f64::abs)
        .filter(|item| *item > 0.0)
}
