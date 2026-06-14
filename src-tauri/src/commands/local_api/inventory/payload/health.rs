use super::builder::{build_inventory_payload, InventoryBuildOptions};
use super::*;

pub(crate) async fn market_data_health(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let symbol_filter = param_string(req, "symbol", "");
    let include_orphans = param_bool(req, "include_orphans", false);
    let inventory = build_inventory_payload(
        state,
        InventoryBuildOptions {
            symbol_filter: (!symbol_filter.trim().is_empty()).then(|| symbol_filter.clone()),
            ..Default::default()
        },
    )
    .await?;

    let rows = inventory
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("库存健康 payload 缺少 rows 数组".to_string()))?;
    let mut health_rows = Vec::new();
    for row in rows {
        let mut row = row.clone();
        if annotate_health_row(&mut row, &symbol_filter, include_orphans)? {
            health_rows.push(row);
        }
    }

    Ok(code_ok(json!({
        "summary": {
            "symbol_count": health_rows.len(),
            "healthy_count": health_rows.iter().filter(|row| row.get("status").and_then(Value::as_str) == Some("healthy")).count(),
            "degraded_count": health_rows.iter().filter(|row| row.get("status").and_then(Value::as_str) == Some("degraded")).count(),
            "missing_count": health_rows.iter().filter(|row| row.get("status").and_then(Value::as_str) == Some("missing")).count()
        },
        "rows": health_rows
    })))
}

fn annotate_health_row(
    row: &mut Value,
    symbol_filter: &str,
    include_orphans: bool,
) -> AppResult<bool> {
    let symbol = required_health_str(row, "symbol")?;
    if !symbol_filter.is_empty()
        && normalize_symbol(symbol_filter).as_deref() != normalize_symbol(symbol).as_deref()
    {
        return Ok(false);
    }
    let managed = required_health_bool(row, "managed")?;
    let orphan = required_health_bool(row, "orphan")?;
    if !include_orphans && (orphan || !managed) {
        return Ok(false);
    }
    let candle_count = required_health_i64(row, "candle_count")?;
    let timeframe_count = required_health_i64(row, "timeframe_record_count")?;
    let missing_timeframes = missing_timeframes(row)?;
    let has_gaps = !missing_timeframes.is_empty();
    let status = if candle_count <= 0 {
        "missing"
    } else if timeframe_count < 2 || has_gaps {
        "degraded"
    } else {
        "healthy"
    };
    let health_score = match status {
        "healthy" => 100,
        "degraded" => 60,
        _ => 0,
    };
    if let Some(obj) = row.as_object_mut() {
        obj.insert("status".to_string(), Value::String(status.to_string()));
        obj.insert(
            "health_score".to_string(),
            Value::Number(health_score.into()),
        );
        obj.insert("has_local_data".to_string(), Value::Bool(candle_count > 0));
        obj.insert(
            "missing_timeframes".to_string(),
            Value::Array(missing_timeframes),
        );
    } else {
        return Err(AppError::Validation(
            "库存健康行必须是 JSON 对象".to_string(),
        ));
    }
    Ok(true)
}

fn missing_timeframes(row: &Value) -> AppResult<Vec<Value>> {
    let markets = row
        .get("markets")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Validation("库存健康行缺少 markets 对象".to_string()))?;
    let mut missing = Vec::new();
    for market in markets.values() {
        let inst_id = required_health_str(market, "inst_id")?;
        let inst_type = required_health_str(market, "inst_type")?;
        let timeframes = market
            .get("timeframes")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Validation("库存健康市场缺少 timeframes 数组".to_string()))?;
        for timeframe in timeframes {
            let gap_count = required_health_i64(timeframe, "gap_count")?;
            if gap_count <= 0 {
                continue;
            }
            let timeframe_label = required_health_str(timeframe, "timeframe")?;
            let coverage_ratio = timeframe.get("coverage_ratio").cloned().ok_or_else(|| {
                AppError::Validation("库存健康周期缺少 coverage_ratio".to_string())
            })?;
            missing.push(json!({
                "inst_id": inst_id,
                "inst_type": inst_type,
                "timeframe": timeframe_label,
                "gap_count": gap_count,
                "coverage_ratio": coverage_ratio
            }));
        }
    }
    Ok(missing)
}

fn required_health_str<'a>(value: &'a Value, key: &str) -> AppResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation(format!("库存健康 payload 字段 {key} 必须是字符串")))
}

fn required_health_bool(value: &Value, key: &str) -> AppResult<bool> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| AppError::Validation(format!("库存健康 payload 字段 {key} 必须是布尔值")))
}

fn required_health_i64(value: &Value, key: &str) -> AppResult<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Validation(format!("库存健康 payload 字段 {key} 必须是整数")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn annotate_health_row_rejects_missing_generated_symbol() {
        let mut row = json!({
            "managed": true,
            "orphan": false,
            "candle_count": 10,
            "timeframe_record_count": 2,
            "markets": {}
        });

        let result = annotate_health_row(&mut row, "", false);

        assert!(result.is_err());
    }

    #[test]
    fn missing_timeframes_rejects_missing_generated_gap_count() {
        let row = json!({
            "markets": {
                "SPOT": {
                    "inst_id": "BTC-USDT",
                    "inst_type": "SPOT",
                    "timeframes": [
                        {
                            "timeframe": "1m",
                            "coverage_ratio": 1.0
                        }
                    ]
                }
            }
        });

        let result = missing_timeframes(&row);

        assert!(result.is_err());
    }
}
