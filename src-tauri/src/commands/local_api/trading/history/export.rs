use super::super::values::finite_text_f64;
use super::super::*;
use super::fills::local_fill_rows;

/// GET /api/trading/performance/export — 导出交易记录为 CSV
pub(crate) async fn trade_performance_export(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let items = local_fill_rows(state, req).await?;

    let mut csv = String::from("时间,交易对,方向,价格,数量,成交金额,手续费\n");
    for item in &items {
        csv.push_str(&trade_csv_row(item)?);
    }

    Ok(json!({
        "code": 0,
        "message": "success",
        "data": {
            "csv": csv,
            "filename": format!("trade_performance_{}.csv", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
            "row_count": items.len()
        }
    }))
}

fn trade_csv_row(item: &Value) -> AppResult<String> {
    let ts_val = required_i64(item, "ts")?;
    let ts = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts_val)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts_val.to_string());
    let inst_id = required_str(item, "inst_id")?;
    let side = required_str(item, "side")?;
    let price = optional_number_text(item, "fill_px")?;
    let size = optional_number_text(item, "fill_sz")?;
    let fill_value = match (finite_text_f64(&price), finite_text_f64(&size)) {
        (Some(price), Some(size)) => (price * size).to_string(),
        _ => String::new(),
    };
    let fee = optional_number_text(item, "fee")?;
    Ok(format!(
        "{ts},{inst_id},{side},{price},{size},{fill_value},{fee}\n"
    ))
}

fn required_i64(item: &Value, key: &str) -> AppResult<i64> {
    item.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| crate::error::AppError::Runtime(format!("交易导出缺少 {key} 整数")))
}

fn required_str<'a>(item: &'a Value, key: &str) -> AppResult<&'a str> {
    item.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| crate::error::AppError::Runtime(format!("交易导出缺少 {key} 字符串")))
}

fn optional_number_text(item: &Value, key: &str) -> AppResult<String> {
    let value = item
        .get(key)
        .ok_or_else(|| crate::error::AppError::Runtime(format!("交易导出缺少 {key} 字段")))?;
    match value {
        Value::Null => Ok(String::new()),
        Value::Number(item) => {
            let text = item.to_string();
            if finite_text_f64(&text).is_some() {
                Ok(text)
            } else {
                Err(crate::error::AppError::Runtime(format!(
                    "交易导出 {key} 不是有限数字"
                )))
            }
        }
        _ => Err(crate::error::AppError::Runtime(format!(
            "交易导出 {key} 不是数字或 null"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn trade_csv_row_does_not_fabricate_zero_for_unknown_price_or_size() {
        let row = trade_csv_row(&json!({
            "ts": 0,
            "inst_id": "BTC-USDT-SWAP",
            "side": "buy",
            "fill_px": null,
            "fill_sz": null,
            "fee": null,
        }))
        .expect("csv row");

        assert_eq!(row, "1970-01-01 00:00:00,BTC-USDT-SWAP,buy,,,,\n");
    }

    #[test]
    fn trade_csv_row_rejects_legacy_alias_only_rows() {
        let error = trade_csv_row(&json!({
            "timestamp": 0,
            "symbol": "BTC-USDT-SWAP",
            "side": "buy",
            "price": 100,
            "size": 2,
            "fill_fee": -0.1,
        }))
        .unwrap_err();

        assert!(error.to_string().contains("ts"));
    }
}
