use serde_json::{json, Map, Value};
use sqlx::SqlitePool;

use crate::market_candle_rows::load_latest_basic_candle_closes;

use super::{
    super::*,
    conditions::evaluate_scan_condition,
    profiles::fetch_scanner_profile,
    symbols::{
        enabled_scanner_symbol_ids, normalize_requested_scanner_symbols, resolve_scanner_inst_type,
    },
};

pub(in crate::commands::local_api) async fn run_scan(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let symbols = request_string_array(req, "symbols");
    let conditions = body_array(req, "conditions");
    let logic = body_string(req, "logic", "and");
    let timeframe = body_string(req, "timeframe", "1H");
    let inst_type = resolve_scanner_inst_type(state, &body_string(req, "inst_type", "")).await?;
    let effective_symbols = if symbols.is_empty() {
        enabled_scanner_symbol_ids(state, &inst_type).await?
    } else {
        normalize_requested_scanner_symbols(state, &symbols, &inst_type).await?
    };
    let results = scan_symbols(
        state,
        &effective_symbols,
        &conditions,
        &logic,
        &timeframe,
        &inst_type,
    )
    .await?;
    let scanned = effective_symbols.len();
    let matched = results.len();
    Ok(code_ok(json!({
        "results": results,
        "scanned": scanned,
        "matched": matched
    })))
}

pub(in crate::commands::local_api) async fn run_profile_scan(
    state: &AppState,
    profile_id: &str,
) -> AppResult<Value> {
    let Some(profile) = fetch_scanner_profile(&state.db, profile_id).await? else {
        return Err(AppError::Validation("扫描方案不存在".to_string()));
    };
    let symbols = profile_string_array(&profile, "symbols")?;
    let inst_type = profile_str(&profile, "inst_type")?;
    let inst_type = resolve_scanner_inst_type(state, inst_type).await?;
    let timeframe = profile_str(&profile, "timeframe")?;
    let logic = profile_str(&profile, "logic")?;
    let conditions = profile_array(&profile, "conditions")?.to_vec();
    let effective_symbols = if symbols.is_empty() {
        enabled_scanner_symbol_ids(state, &inst_type).await?
    } else {
        normalize_requested_scanner_symbols(state, &symbols, &inst_type).await?
    };
    let results = scan_symbols(
        state,
        &effective_symbols,
        &conditions,
        logic,
        timeframe,
        &inst_type,
    )
    .await?;
    for result in &results {
        sqlx::query(
            r#"
            INSERT INTO scanner_results (
              profile_id, inst_id, inst_type, timeframe,
              matched_conditions_json, indicator_values_json, price, scan_time
            ) VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(profile_id)
        .bind(required_str(result, "inst_id")?)
        .bind(&inst_type)
        .bind(timeframe)
        .bind(serde_json::to_string(required_value(
            result,
            "matched_conditions",
        )?)?)
        .bind(serde_json::to_string(required_value(
            result,
            "indicator_values",
        )?)?)
        .bind(required_f64(result, "price")?)
        .execute(&state.db)
        .await?;
    }
    let scanned = effective_symbols.len();
    let matched = results.len();
    Ok(code_ok(json!({
        "results": results,
        "scanned": scanned,
        "matched": matched
    })))
}

async fn scan_symbols(
    state: &AppState,
    symbols: &[String],
    conditions: &[Value],
    logic: &str,
    timeframe: &str,
    inst_type: &str,
) -> AppResult<Vec<Value>> {
    let mut results = Vec::new();
    for symbol in symbols {
        super::super::market_ops::ensure_local_candles_for_read(
            state, symbol, inst_type, timeframe, 120, false,
        )
        .await?;
        let closes = load_scan_closes(&state.db, symbol, inst_type, timeframe, 120).await?;
        if closes.len() < 2 {
            continue;
        }
        let price = closes[closes.len() - 1];
        let mut matched_conditions = Vec::new();
        let mut indicator_values = Map::new();
        indicator_values.insert("price".to_string(), json!(price));
        for condition in conditions {
            let indicator = condition_str(condition, "indicator")?;
            let operator = condition_str(condition, "operator")?;
            let target = condition_f64(condition, "value")?;
            let params = condition.get("params").and_then(Value::as_object);
            let (passed, name, value) =
                evaluate_scan_condition(indicator, operator, target, params, &closes)?;
            if let Some(value) = value {
                indicator_values.insert(name.clone(), json!(value));
            }
            if passed {
                matched_conditions.push(Value::String(name));
            }
        }
        let matched = if conditions.is_empty() {
            true
        } else if logic == "or" {
            !matched_conditions.is_empty()
        } else {
            matched_conditions.len() == conditions.len()
        };
        if matched {
            results.push(json!({
                "inst_id": symbol,
                "inst_type": inst_type,
                "timeframe": timeframe,
                "matched_conditions": matched_conditions,
                "indicator_values": indicator_values,
                "price": price
            }));
        }
    }
    Ok(results)
}

fn profile_array<'a>(value: &'a Value, field: &str) -> AppResult<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Runtime(format!("scanner profile 字段 {field} 不是数组")))
}

fn profile_string_array(value: &Value, field: &str) -> AppResult<Vec<String>> {
    profile_array(value, field)?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    AppError::Runtime(format!(
                        "scanner profile 字段 {field} 包含非空字符串以外的值"
                    ))
                })
        })
        .collect()
}

fn profile_str<'a>(value: &'a Value, field: &str) -> AppResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| AppError::Runtime(format!("scanner profile 字段 {field} 不是非空字符串")))
}

fn condition_str<'a>(value: &'a Value, field: &str) -> AppResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("扫描条件缺少非空字符串字段 {field}")))
}

fn condition_f64(value: &Value, field: &str) -> AppResult<f64> {
    let number = value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| AppError::Validation(format!("扫描条件缺少数字字段 {field}")))?;
    if !number.is_finite() {
        return Err(AppError::Validation(format!(
            "扫描条件字段 {field} 不是有限数字"
        )));
    }
    Ok(number)
}

fn required_value<'a>(value: &'a Value, field: &str) -> AppResult<&'a Value> {
    value
        .get(field)
        .ok_or_else(|| AppError::Runtime(format!("scanner result 缺少字段 {field}")))
}

fn required_str<'a>(value: &'a Value, field: &str) -> AppResult<&'a str> {
    required_value(value, field)?
        .as_str()
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| AppError::Runtime(format!("scanner result 字段 {field} 不是非空字符串")))
}

fn required_f64(value: &Value, field: &str) -> AppResult<f64> {
    let number = required_value(value, field)?
        .as_f64()
        .ok_or_else(|| AppError::Runtime(format!("scanner result 字段 {field} 不是数字")))?;
    if !number.is_finite() {
        return Err(AppError::Runtime(format!(
            "scanner result 字段 {field} 不是有限数字"
        )));
    }
    Ok(number)
}

async fn load_scan_closes(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<f64>> {
    load_latest_basic_candle_closes(db, symbol, inst_type, timeframe, limit)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::query(
            r#"
            CREATE TABLE candles (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SPOT',
              timeframe TEXT NOT NULL,
              timestamp INTEGER NOT NULL,
              open REAL NOT NULL,
              high REAL NOT NULL,
              low REAL NOT NULL,
              close REAL NOT NULL,
              volume REAL NOT NULL,
              volume_ccy REAL DEFAULT 0,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles table");
        pool
    }

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn scan_closes_skip_invalid_market_rows_instead_of_fabricating_zero() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "'bad-close'").await;
        insert_candle(&pool, 180_000, "102").await;

        let closes = load_scan_closes(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 120)
            .await
            .expect("load scan closes");

        assert_eq!(closes, vec![100.0, 102.0]);
    }

    #[tokio::test]
    async fn scan_closes_applies_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let closes = load_scan_closes(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load scan closes");

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
