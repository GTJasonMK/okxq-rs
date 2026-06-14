use super::super::*;
use sqlx::{sqlite::SqliteRow, Row};

pub(crate) async fn trading_fee_rates(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "SWAP");
    let inst_id = param_string(req, "inst_id", "");
    let inst_family = param_string(req, "inst_family", "");
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_trade_fee(&inst_type, &inst_id, &inst_family)
        .await?;
    Ok(json!({
        "mode": mode,
        "inst_type": inst_type.trim().to_uppercase(),
        "inst_id": inst_id.trim().to_uppercase(),
        "inst_family": inst_family.trim().to_uppercase(),
        "source": "okx_account_trade_fee",
        "data": items,
    }))
}

pub(crate) async fn sync_trading_fee_rates(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = request_string(req, "inst_type", "SWAP");
    let inst_ids = parse_fee_rate_inst_ids(&request_string(req, "inst_id", ""));
    let inst_family = request_string(req, "inst_family", "");
    let targets = if inst_ids.is_empty() {
        vec![String::new()]
    } else {
        inst_ids
    };
    let client = okx_private_client(state, &mode).await?;
    let fetched_at = chrono::Utc::now().timestamp_millis();
    let mut request_count = 0_i64;
    let mut row_count = 0_i64;
    for inst_id in targets {
        let items = client
            .get_trade_fee(&inst_type, &inst_id, &inst_family)
            .await?;
        request_count += 1;
        for item in items {
            upsert_fee_rate_row(
                &state.db,
                &mode,
                &inst_type,
                &inst_id,
                &inst_family,
                &item,
                fetched_at,
            )
            .await?;
            row_count += 1;
        }
    }
    Ok(json!({
        "mode": mode,
        "inst_type": inst_type.trim().to_uppercase(),
        "inst_family": inst_family.trim().to_uppercase(),
        "request_count": request_count,
        "stored": row_count,
        "source": "okx_account_trade_fee",
        "fetched_at": fetched_at,
        "note": "Stored OKX account trade-fee rows for fee-schedule evidence. Fill-level fee evidence still requires local_fills."
    }))
}

pub(crate) async fn local_trading_fee_rates(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "");
    let inst_id = param_string(req, "inst_id", "");
    let limit = param_i64(req, "limit", 100).clamp(1, 500);
    let mut sql = "SELECT * FROM okx_account_fee_rates WHERE mode = ?".to_string();
    if !inst_type.is_empty() {
        sql.push_str(" AND inst_type = ?");
    }
    if !inst_id.is_empty() {
        sql.push_str(" AND inst_id = ?");
    }
    sql.push_str(" ORDER BY fetched_at DESC, inst_type ASC, inst_id ASC LIMIT ?");
    let mut query = sqlx::query(&sql).bind(mode);
    if !inst_type.is_empty() {
        query = query.bind(inst_type.trim().to_uppercase());
    }
    if !inst_id.is_empty() {
        query = query.bind(inst_id.trim().to_uppercase());
    }
    query = query.bind(limit);
    let rows = query.fetch_all(&state.db).await?;
    let rates = rows
        .into_iter()
        .map(fee_rate_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Value::Array(rates))
}

fn fee_rate_row_to_json(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "mode": row.try_get::<String, _>("mode")?,
        "inst_type": row.try_get::<String, _>("inst_type")?,
        "inst_id": row.try_get::<String, _>("inst_id")?,
        "inst_family": row.try_get::<String, _>("inst_family")?,
        "maker_rate": row.try_get::<Option<f64>, _>("maker_rate")?,
        "taker_rate": row.try_get::<Option<f64>, _>("taker_rate")?,
        "maker_u_rate": row.try_get::<Option<f64>, _>("maker_u_rate")?,
        "taker_u_rate": row.try_get::<Option<f64>, _>("taker_u_rate")?,
        "maker_usdc_rate": row.try_get::<Option<f64>, _>("maker_usdc_rate")?,
        "taker_usdc_rate": row.try_get::<Option<f64>, _>("taker_usdc_rate")?,
        "level": row.try_get::<String, _>("level")?,
        "payload": fee_payload_json(&row)?,
        "fetched_at": row.try_get::<i64, _>("fetched_at")?,
        "source": row.try_get::<String, _>("source")?,
    }))
}

fn fee_payload_json(row: &SqliteRow) -> AppResult<Value> {
    let text = row.try_get::<String, _>("payload_json")?;
    let value = serde_json::from_str::<Value>(&text)?;
    match value {
        Value::Object(_) => Ok(value),
        _ => Err(AppError::Runtime(
            "okx_account_fee_rates payload_json 不是 JSON 对象".to_string(),
        )),
    }
}

fn parse_fee_rate_inst_ids(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_uppercase())
        .filter(|item| !item.is_empty())
        .collect()
}

async fn upsert_fee_rate_row(
    db: &sqlx::SqlitePool,
    mode: &str,
    request_inst_type: &str,
    request_inst_id: &str,
    request_inst_family: &str,
    item: &Value,
    fetched_at: i64,
) -> AppResult<()> {
    let inst_type =
        fee_text(item, &["instType"]).unwrap_or_else(|| request_inst_type.trim().to_uppercase());
    let inst_id =
        fee_text(item, &["instId"]).unwrap_or_else(|| request_inst_id.trim().to_uppercase());
    let inst_family = fee_text(item, &["instFamily", "uly"])
        .unwrap_or_else(|| request_inst_family.trim().to_uppercase());
    let payload_json = serde_json::to_string(item)
        .map_err(|error| AppError::Runtime(format!("OKX fee-rate payload 序列化失败: {error}")))?;
    sqlx::query(
        r#"
        INSERT INTO okx_account_fee_rates (
          mode, inst_type, inst_id, inst_family,
          maker_rate, taker_rate, maker_u_rate, taker_u_rate,
          maker_usdc_rate, taker_usdc_rate, level, payload_json, fetched_at, source
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'okx_account_trade_fee')
        ON CONFLICT(mode, inst_type, inst_id, inst_family) DO UPDATE SET
          maker_rate = excluded.maker_rate,
          taker_rate = excluded.taker_rate,
          maker_u_rate = excluded.maker_u_rate,
          taker_u_rate = excluded.taker_u_rate,
          maker_usdc_rate = excluded.maker_usdc_rate,
          taker_usdc_rate = excluded.taker_usdc_rate,
          level = excluded.level,
          payload_json = excluded.payload_json,
          fetched_at = excluded.fetched_at,
          source = excluded.source
        "#,
    )
    .bind(mode)
    .bind(inst_type)
    .bind(inst_id)
    .bind(inst_family)
    .bind(fee_f64(item, &["maker"]))
    .bind(fee_f64(item, &["taker"]))
    .bind(fee_f64(item, &["makerU"]))
    .bind(fee_f64(item, &["takerU"]))
    .bind(fee_f64(item, &["makerUSDC"]))
    .bind(fee_f64(item, &["takerUSDC"]))
    .bind(fee_text(item, &["level"]).unwrap_or_default())
    .bind(payload_json)
    .bind(fetched_at)
    .execute(db)
    .await?;
    Ok(())
}

fn fee_text(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| item.get(*key))
        .and_then(value_as_trimmed_string)
}

fn fee_f64(item: &Value, keys: &[&str]) -> Option<f64> {
    fee_text(item, keys).and_then(|value| value.parse::<f64>().ok())
}

fn value_as_trimmed_string(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(item) => item.clone(),
        Value::Number(item) => item.to_string(),
        Value::Bool(item) => item.to_string(),
        _ => return None,
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fee_rate_inst_ids_accepts_comma_separated_symbols() {
        assert_eq!(
            parse_fee_rate_inst_ids(" eth-usdt-swap, SOL-USDT-SWAP ,, "),
            vec!["ETH-USDT-SWAP".to_string(), "SOL-USDT-SWAP".to_string()]
        );
    }

    #[test]
    fn fee_rate_helpers_parse_okx_string_rates() {
        let item = json!({
            "maker": "-0.0001",
            "taker": "0.0005",
            "level": "Lv1"
        });

        assert_eq!(fee_f64(&item, &["maker"]), Some(-0.0001));
        assert_eq!(fee_f64(&item, &["taker"]), Some(0.0005));
        assert_eq!(fee_text(&item, &["level"]), Some("Lv1".to_string()));
    }

    #[tokio::test]
    async fn fee_rate_row_rejects_invalid_payload_json() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              'simulated' AS mode,
              'SWAP' AS inst_type,
              'BTC-USDT-SWAP' AS inst_id,
              '' AS inst_family,
              NULL AS maker_rate,
              NULL AS taker_rate,
              NULL AS maker_u_rate,
              NULL AS taker_u_rate,
              NULL AS maker_usdc_rate,
              NULL AS taker_usdc_rate,
              'Lv1' AS level,
              'not-json' AS payload_json,
              1700000000000 AS fetched_at,
              'okx_account_trade_fee' AS source
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("fee row");

        assert!(fee_rate_row_to_json(row).is_err());
    }
}
