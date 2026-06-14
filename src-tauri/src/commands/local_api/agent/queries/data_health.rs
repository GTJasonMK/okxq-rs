use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    app_state::AppState,
    commands::local_api::{body_string, code_ok, normalize_symbol, LocalApiRequest},
    error::AppResult,
    sync_record_summary::{
        load_sync_record_market_aggregates_for_scopes, watched_market_scopes,
        SyncRecordMarketAggregate,
    },
};

/// POST /api/agent/query/data-health
pub(crate) async fn query_data_health(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let symbol = body_string(req, "symbol", "");
    let watched = state.preferences.watched_symbols().await?;
    let allowed_scopes = watched_market_scopes(&watched);
    let Ok(symbol_filter) = normalized_symbol_filter(&symbol) else {
        return Ok(code_ok(data_health_payload(Vec::new())));
    };
    let rows =
        sync_record_health_rows_from_pool(&state.db, &allowed_scopes, symbol_filter.as_deref())
            .await?;
    Ok(code_ok(data_health_payload(rows)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DataHealthRow {
    inst_id: String,
    normalized_symbol: String,
    candle_count: i64,
    timeframe_count: i64,
}

async fn sync_record_health_rows_from_pool(
    pool: &SqlitePool,
    allowed_scopes: &BTreeSet<(String, String)>,
    symbol_filter: Option<&str>,
) -> AppResult<Vec<DataHealthRow>> {
    let target_scopes = target_health_scopes(allowed_scopes, symbol_filter);
    if target_scopes.is_empty() {
        return Ok(Vec::new());
    }
    let rows = load_sync_record_market_aggregates_for_scopes(pool, &target_scopes).await?;
    Ok(rows
        .into_iter()
        .filter_map(data_health_row_from_aggregate)
        .collect())
}

fn target_health_scopes(
    allowed_scopes: &BTreeSet<(String, String)>,
    symbol_filter: Option<&str>,
) -> BTreeSet<(String, String)> {
    allowed_scopes
        .iter()
        .filter(|(inst_id, _)| {
            symbol_filter.map_or(true, |symbol| {
                normalize_symbol(inst_id).as_deref() == Some(symbol)
            })
        })
        .cloned()
        .collect()
}

fn data_health_row_from_aggregate(record: SyncRecordMarketAggregate) -> Option<DataHealthRow> {
    Some(DataHealthRow {
        normalized_symbol: normalize_symbol(&record.inst_id)?,
        inst_id: record.inst_id,
        candle_count: record.candle_count,
        timeframe_count: record.timeframe_count,
    })
}

fn normalized_symbol_filter(symbol: &str) -> Result<Option<String>, ()> {
    if symbol.trim().is_empty() {
        return Ok(None);
    }
    normalize_symbol(symbol).map(Some).ok_or(())
}

fn data_health_payload(rows: impl IntoIterator<Item = DataHealthRow>) -> Value {
    let mut health_map: BTreeMap<String, Value> = BTreeMap::new();
    for row in rows {
        let entry = health_map.entry(row.normalized_symbol).or_insert_with(|| {
            json!({"symbol": "", "candle_count": 0, "timeframe_count": 0, "status": "missing", "health_score": 0})
        });
        if let Some(obj) = entry.as_object_mut() {
            if obj
                .get("symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
            {
                obj.insert("symbol".to_string(), json!(row.inst_id));
            }
            obj.insert("managed".to_string(), json!(true));
            let current = obj
                .get("candle_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            obj.insert(
                "candle_count".to_string(),
                json!(current + row.candle_count.max(0)),
            );
            let tf = obj
                .get("timeframe_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            obj.insert(
                "timeframe_count".to_string(),
                json!(tf + row.timeframe_count.max(0)),
            );
        }
    }

    for (_sym, value) in health_map.iter_mut() {
        if let Some(obj) = value.as_object_mut() {
            let candle_count = obj
                .get("candle_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let tf_count = obj
                .get("timeframe_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let (status, score) = if candle_count <= 0 {
                ("missing", 0)
            } else if tf_count < 2 {
                ("degraded", 60)
            } else {
                ("healthy", 100)
            };
            obj.insert("status".to_string(), json!(status));
            obj.insert("health_score".to_string(), json!(score));
        }
    }

    let items: Vec<Value> = health_map.into_values().collect();
    json!({
        "total": items.len(),
        "items": items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn data_health_symbol_query_uses_scoped_sync_record_aggregates() {
        let pool = memory_pool().await;
        create_sync_records_table(&pool).await;
        insert_sync_record_rows(
            &pool,
            &[
                ("BTC-USDT", "SPOT", "1m", 100),
                ("BTC-USDT", "SPOT", "5m", 30),
                ("BTC-USDT", "SPOT", "15m", -40),
                ("BTC-USDT-SWAP", "SWAP", "1m", 80),
                ("ETH-USDT", "SPOT", "1m", 15),
                ("DOGE-USDT", "SPOT", "1m", 999),
            ],
        )
        .await;
        let allowed = BTreeSet::from([
            ("BTC-USDT".to_string(), "SPOT".to_string()),
            ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
            ("ETH-USDT".to_string(), "SPOT".to_string()),
        ]);
        let symbol = normalized_symbol_filter("BTC-USDT").unwrap().unwrap();

        let rows = sync_record_health_rows_from_pool(&pool, &allowed, Some(symbol.as_str()))
            .await
            .expect("data health rows");
        assert_eq!(
            rows,
            vec![
                DataHealthRow {
                    inst_id: "BTC-USDT".to_string(),
                    normalized_symbol: "BTC-USDT".to_string(),
                    candle_count: 130,
                    timeframe_count: 3,
                },
                DataHealthRow {
                    inst_id: "BTC-USDT-SWAP".to_string(),
                    normalized_symbol: "BTC-USDT".to_string(),
                    candle_count: 80,
                    timeframe_count: 1,
                },
            ]
        );

        let payload = data_health_payload(rows);
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["symbol"], "BTC-USDT");
        assert_eq!(payload["items"][0]["candle_count"], 210);
        assert_eq!(payload["items"][0]["timeframe_count"], 4);
        assert_eq!(payload["items"][0]["status"], "healthy");
    }

    #[tokio::test]
    async fn data_health_invalid_symbol_filter_returns_empty_instead_of_all_scopes() {
        let pool = memory_pool().await;
        create_sync_records_table(&pool).await;
        insert_sync_record_rows(&pool, &[("BTC-USDT", "SPOT", "1m", 100)]).await;
        let allowed = BTreeSet::from([("BTC-USDT".to_string(), "SPOT".to_string())]);
        let rows = match normalized_symbol_filter("   ###   ") {
            Ok(symbol) => sync_record_health_rows_from_pool(&pool, &allowed, symbol.as_deref())
                .await
                .expect("optimized data health"),
            Err(()) => Vec::new(),
        };

        assert_eq!(
            data_health_payload(rows),
            json!({ "total": 0, "items": [] })
        );
    }

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    async fn create_sync_records_table(pool: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE sync_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT NOT NULL,
                inst_type TEXT NOT NULL DEFAULT 'SPOT',
                timeframe TEXT NOT NULL,
                candle_count INTEGER DEFAULT 0,
                UNIQUE(inst_id, inst_type, timeframe)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create sync_records table");
    }

    async fn insert_sync_record_rows(pool: &SqlitePool, rows: &[(&str, &str, &str, i64)]) {
        let mut tx = pool.begin().await.expect("begin tx");
        for (inst_id, inst_type, timeframe, candle_count) in rows {
            sqlx::query(
                "INSERT INTO sync_records (inst_id, inst_type, timeframe, candle_count) VALUES (?, ?, ?, ?)",
            )
            .bind(inst_id)
            .bind(inst_type)
            .bind(timeframe)
            .bind(candle_count)
            .execute(&mut *tx)
            .await
            .expect("insert sync record");
        }
        tx.commit().await.expect("commit tx");
    }
}
