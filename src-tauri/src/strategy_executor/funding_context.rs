use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::error::{AppError, AppResult};

use super::context::{
    RuntimeCandleRequirement, RuntimeFundingRequirement, RuntimeOrderbookRequirement,
};

const MAX_LOCAL_FUNDING_HISTORY_LIMIT: usize = 20_000;

pub(crate) fn normalize_runtime_inst_id(raw: &str, inst_type: &str) -> AppResult<String> {
    let mut value = raw.trim().to_uppercase();
    if value.is_empty() {
        return Err(AppError::Validation("交易对不能为空".to_string()));
    }
    if !value.contains('-') {
        value = format!("{value}-USDT");
    }
    if inst_type.eq_ignore_ascii_case("SWAP") && !value.ends_with("-SWAP") {
        value = format!("{value}-SWAP");
    }
    if inst_type.eq_ignore_ascii_case("SPOT") && value.ends_with("-SWAP") {
        value.truncate(value.len() - 5);
    }
    Ok(value)
}

pub(crate) fn normalize_funding_requirement(
    requirement: RuntimeFundingRequirement,
) -> AppResult<RuntimeFundingRequirement> {
    Ok(RuntimeFundingRequirement {
        symbol: normalize_runtime_inst_id(&requirement.symbol, &requirement.inst_type)?,
        inst_type: requirement.inst_type,
        history_limit: requirement.history_limit,
        required: requirement.required,
    })
}

pub(crate) fn normalize_candle_requirement(
    requirement: RuntimeCandleRequirement,
) -> AppResult<RuntimeCandleRequirement> {
    let symbol = normalize_runtime_inst_id(&requirement.symbol, &requirement.inst_type)?;
    let timeframe = crate::timeframes::normalize_okx_timeframe(&requirement.timeframe)
        .ok_or_else(|| {
            AppError::Validation(format!(
                "{} {} DATA_REQUIREMENTS 声明了不支持的 K 线周期: {}",
                symbol,
                requirement.inst_type,
                requirement.timeframe.trim()
            ))
        })?
        .to_string();
    Ok(RuntimeCandleRequirement {
        symbol,
        inst_type: requirement.inst_type,
        timeframe,
        min_bars: requirement.min_bars,
        role: requirement.role,
    })
}

pub(crate) fn normalize_orderbook_requirement(
    requirement: RuntimeOrderbookRequirement,
) -> AppResult<RuntimeOrderbookRequirement> {
    Ok(RuntimeOrderbookRequirement {
        symbol: normalize_runtime_inst_id(&requirement.symbol, &requirement.inst_type)?,
        inst_type: requirement.inst_type,
        depth: requirement.depth,
        required: requirement.required,
    })
}

pub(crate) async fn local_funding_table_exists(db: &SqlitePool) -> AppResult<bool> {
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind("okx_funding_rates")
        .fetch_optional(db)
        .await?;
    Ok(row.is_some())
}

pub(crate) async fn load_local_funding_history_checked(
    db: &SqlitePool,
    requirement: &RuntimeFundingRequirement,
    timestamp: i64,
    limit: usize,
    table_exists: bool,
) -> AppResult<Vec<Value>> {
    if !table_exists {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT inst_id, inst_type, funding_time, funding_rate, realized_rate,
               method, formula_type, payload_json, fetched_at
        FROM (
          SELECT inst_id, inst_type, funding_time, funding_rate, realized_rate,
                 method, formula_type, payload_json, fetched_at
          FROM okx_funding_rates
          WHERE inst_id = ? AND inst_type = ? AND (? <= 0 OR funding_time <= ?)
            AND typeof(funding_time) = 'integer'
            AND funding_time > 0
            AND typeof(funding_rate) IN ('integer', 'real')
          ORDER BY funding_time DESC
          LIMIT ?
        )
        ORDER BY funding_time ASC
        "#,
    )
    .bind(&requirement.symbol)
    .bind(&requirement.inst_type)
    .bind(timestamp)
    .bind(timestamp)
    .bind(limit.clamp(1, MAX_LOCAL_FUNDING_HISTORY_LIMIT) as i64)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().filter_map(funding_row_to_value).collect())
}

pub(crate) async fn load_local_funding_history_until_checked(
    db: &SqlitePool,
    requirement: &RuntimeFundingRequirement,
    timestamp: i64,
    table_exists: bool,
) -> AppResult<Vec<Value>> {
    if !table_exists {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT inst_id, inst_type, funding_time, funding_rate, realized_rate,
               method, formula_type, payload_json, fetched_at
        FROM okx_funding_rates
        WHERE inst_id = ? AND inst_type = ? AND (? <= 0 OR funding_time <= ?)
          AND typeof(funding_time) = 'integer'
          AND funding_time > 0
          AND typeof(funding_rate) IN ('integer', 'real')
        ORDER BY funding_time ASC
        "#,
    )
    .bind(&requirement.symbol)
    .bind(&requirement.inst_type)
    .bind(timestamp)
    .bind(timestamp)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().filter_map(funding_row_to_value).collect())
}

pub(crate) fn funding_context_value_from_history(source: &str, history: Vec<Value>) -> Value {
    json!({
        "source": source,
        "latest": history.last().cloned().unwrap_or_else(|| json!({})),
        "history": history,
    })
}

fn funding_row_to_value(row: sqlx::sqlite::SqliteRow) -> Option<Value> {
    let funding_time = row.try_get::<i64, _>("funding_time").ok()?;
    if funding_time <= 0 {
        return None;
    }
    let funding_rate = row.try_get::<f64, _>("funding_rate").ok()?;
    if !funding_rate.is_finite() {
        return None;
    }
    let realized_rate = row.try_get::<Option<f64>, _>("realized_rate").ok()?;
    if realized_rate.is_some_and(|value| !value.is_finite()) {
        return None;
    }
    let payload_json =
        serde_json::from_str::<Value>(&row.try_get::<String, _>("payload_json").ok()?).ok()?;
    Some(json!({
        "inst_id": row.try_get::<String, _>("inst_id").ok()?,
        "inst_type": row.try_get::<String, _>("inst_type").ok()?,
        "funding_time": funding_time,
        "timestamp": funding_time,
        "funding_rate": funding_rate,
        "rate": funding_rate,
        "realized_rate": realized_rate,
        "method": row.try_get::<String, _>("method").ok()?,
        "formula_type": row.try_get::<String, _>("formula_type").ok()?,
        "payload": payload_json,
        "fetched_at": row.try_get::<String, _>("fetched_at").ok()?,
    }))
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn local_funding_history_backfills_valid_rows_after_invalid_recent_rate() {
        let pool = test_pool().await;
        let requirement = RuntimeFundingRequirement {
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            history_limit: 3,
            required: true,
        };

        insert_funding_rate(&pool, 1_000, "0.0001").await;
        insert_funding_rate(&pool, 2_000, "0.0002").await;
        insert_funding_rate(&pool, 3_000, "0.0003").await;
        insert_funding_rate(&pool, 4_000, "'bad-rate'").await;

        let table_exists = local_funding_table_exists(&pool)
            .await
            .expect("funding table should be detectable");
        let history =
            load_local_funding_history_checked(&pool, &requirement, 5_000, 3, table_exists)
                .await
                .expect("funding history should load");

        let funding_times = history
            .iter()
            .map(|item| item["funding_time"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(funding_times, vec![1_000, 2_000, 3_000]);
    }

    #[tokio::test]
    async fn local_funding_history_until_loads_valid_rows_through_timestamp() {
        let pool = test_pool().await;
        let requirement = RuntimeFundingRequirement {
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            history_limit: 3,
            required: true,
        };

        insert_funding_rate(&pool, 1_000, "0.0001").await;
        insert_funding_rate(&pool, 2_000, "0.0002").await;
        insert_funding_rate(&pool, 3_000, "0.0003").await;
        insert_funding_rate(&pool, 4_000, "'bad-rate'").await;

        let table_exists = local_funding_table_exists(&pool)
            .await
            .expect("funding table should be detectable");
        let history =
            load_local_funding_history_until_checked(&pool, &requirement, 2_500, table_exists)
                .await
                .expect("funding history should load");

        let funding_times = history
            .iter()
            .map(|item| item["funding_time"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(funding_times, vec![1_000, 2_000]);
    }

    #[tokio::test]
    async fn local_funding_history_rejects_invalid_payload_json() {
        let pool = test_pool().await;
        let requirement = RuntimeFundingRequirement {
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            history_limit: 3,
            required: true,
        };

        insert_funding_rate(&pool, 1_000, "0.0001").await;
        sqlx::query(
            r#"
            INSERT INTO okx_funding_rates
              (inst_id, inst_type, funding_time, funding_rate, payload_json, fetched_at)
            VALUES ('BTC-USDT-SWAP', 'SWAP', 2000, 0.0002, '{bad-json', '')
            "#,
        )
        .execute(&pool)
        .await
        .expect("invalid payload row should insert");

        let table_exists = local_funding_table_exists(&pool)
            .await
            .expect("funding table should be detectable");
        let history =
            load_local_funding_history_until_checked(&pool, &requirement, 3_000, table_exists)
                .await
                .expect("funding history should load");

        let funding_times = history
            .iter()
            .map(|item| item["funding_time"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(funding_times, vec![1_000]);
    }

    #[tokio::test]
    async fn local_funding_history_returns_empty_when_table_is_absent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let requirement = RuntimeFundingRequirement {
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            history_limit: 3,
            required: true,
        };

        let history = load_local_funding_history_checked(&pool, &requirement, 5_000, 3, false)
            .await
            .expect("missing table should be treated as empty when caller allows it");

        assert!(history.is_empty());
    }

    #[test]
    fn normalize_runtime_inst_id_respects_instrument_type() {
        assert_eq!(
            normalize_runtime_inst_id("btc-usdt", "SWAP").unwrap(),
            "BTC-USDT-SWAP"
        );
        assert_eq!(
            normalize_runtime_inst_id("btc-usdt-swap", "SPOT").unwrap(),
            "BTC-USDT"
        );
        assert_eq!(
            normalize_runtime_inst_id("eth", "SPOT").unwrap(),
            "ETH-USDT"
        );
    }

    #[test]
    fn normalize_candle_requirement_centralizes_symbol_and_timeframe_rules() {
        let requirement = RuntimeCandleRequirement {
            symbol: "btc".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "4h".to_string(),
            min_bars: 120,
            role: "confirm".to_string(),
        };

        let normalized = normalize_candle_requirement(requirement).unwrap();

        assert_eq!(normalized.symbol, "BTC-USDT-SWAP");
        assert_eq!(normalized.inst_type, "SWAP");
        assert_eq!(normalized.timeframe, "4H");
        assert_eq!(normalized.min_bars, 120);
        assert_eq!(normalized.role, "confirm");
    }

    #[test]
    fn normalize_candle_requirement_rejects_unknown_timeframe() {
        let requirement = RuntimeCandleRequirement {
            symbol: "btc".to_string(),
            inst_type: "SPOT".to_string(),
            timeframe: "bad".to_string(),
            min_bars: 20,
            role: "primary".to_string(),
        };

        let error = normalize_candle_requirement(requirement)
            .expect_err("unknown timeframe must be rejected")
            .to_string();

        assert!(error.contains("不支持的 K 线周期"));
    }

    #[test]
    fn normalize_orderbook_requirement_centralizes_symbol_rule() {
        let requirement = RuntimeOrderbookRequirement {
            symbol: "btc-usdt-swap".to_string(),
            inst_type: "SPOT".to_string(),
            depth: 50,
            required: false,
        };

        let normalized = normalize_orderbook_requirement(requirement).unwrap();

        assert_eq!(normalized.symbol, "BTC-USDT");
        assert_eq!(normalized.inst_type, "SPOT");
        assert_eq!(normalized.depth, 50);
        assert!(!normalized.required);
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(
            r#"
            CREATE TABLE okx_funding_rates (
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SWAP',
              funding_time INTEGER NOT NULL,
              funding_rate REAL NOT NULL,
              realized_rate REAL,
              method TEXT NOT NULL DEFAULT '',
              formula_type TEXT NOT NULL DEFAULT '',
              payload_json TEXT NOT NULL DEFAULT '{}',
              fetched_at TEXT NOT NULL DEFAULT ''
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create funding table");
        pool
    }

    async fn insert_funding_rate(pool: &SqlitePool, funding_time: i64, funding_rate_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO okx_funding_rates
              (inst_id, inst_type, funding_time, funding_rate, payload_json, fetched_at)
            VALUES ('BTC-USDT-SWAP', 'SWAP', ?, {funding_rate_sql}, '{{}}', '')
            "#
        );
        sqlx::query(&sql)
            .bind(funding_time)
            .execute(pool)
            .await
            .expect("funding row should insert");
    }
}
