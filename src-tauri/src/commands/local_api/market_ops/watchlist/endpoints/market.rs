use std::collections::{BTreeMap, BTreeSet};

use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::config::WatchedSymbolRecord;

use super::super::super::*;

type SymbolScopeKey = (String, String);
type SymbolMeta = (Vec<String>, i64);

pub(in crate::commands::local_api) async fn market_symbols(state: &AppState) -> AppResult<Value> {
    let watched = state.preferences.watched_symbols().await?;
    let enabled_scopes = watched_market_symbol_scopes(&watched);
    let local_meta = load_market_symbol_meta(&state.db, &enabled_scopes).await?;

    let mut data = Vec::new();
    for record in watched {
        for (enabled, inst_id, inst_type) in [
            (record.sync_spot, record.spot_inst_id.as_str(), "SPOT"),
            (record.sync_swap, record.swap_inst_id.as_str(), "SWAP"),
        ] {
            if !enabled {
                continue;
            }
            let normalized_id = inst_id.trim().to_uppercase();
            let normalized_type = inst_type.to_string();
            let (timeframes, candle_count) = local_meta
                .get(&(normalized_id.clone(), normalized_type.clone()))
                .cloned()
                .unwrap_or_default();
            data.push(json!({
                "symbol": record.symbol.clone(),
                "base_ccy": record.base_ccy.clone(),
                "inst_id": normalized_id,
                "inst_type": normalized_type,
                "timeframes": timeframes.into_iter().map(Value::String).collect::<Vec<_>>(),
                "candle_count": candle_count,
                "managed": true,
                "watched": true
            }));
        }
    }
    Ok(code_ok(Value::Array(data)))
}

async fn load_market_symbol_meta(
    db: &SqlitePool,
    enabled_scopes: &BTreeSet<SymbolScopeKey>,
) -> AppResult<BTreeMap<SymbolScopeKey, SymbolMeta>> {
    if enabled_scopes.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut meta = load_market_symbol_meta_from_sync_records(db, enabled_scopes).await?;
    let missing = enabled_scopes
        .iter()
        .filter(|scope| !meta.contains_key(*scope))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        for (scope, item) in load_market_symbol_meta_from_candles(db, &missing).await? {
            meta.insert(scope, item);
        }
    }
    Ok(meta)
}

async fn load_market_symbol_meta_from_sync_records(
    db: &SqlitePool,
    enabled_scopes: &BTreeSet<SymbolScopeKey>,
) -> AppResult<BTreeMap<SymbolScopeKey, SymbolMeta>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
        WITH enabled(inst_id, inst_type) AS (
        "#,
    );
    push_enabled_scope_values(&mut query, enabled_scopes);
    query.push(
        r#"
        )
        SELECT r.inst_id, r.inst_type,
               GROUP_CONCAT(DISTINCT r.timeframe) AS timeframes,
               COALESCE(SUM(r.candle_count), 0) AS candle_count
        FROM enabled e
        INNER JOIN sync_records r
          ON r.inst_id = e.inst_id
         AND r.inst_type = e.inst_type
        WHERE r.candle_count > 0
        GROUP BY r.inst_id, r.inst_type
        ORDER BY r.inst_id
        "#,
    );
    let rows = query.build().fetch_all(db).await?;
    symbol_meta_from_rows(rows)
}

async fn load_market_symbol_meta_from_candles(
    db: &SqlitePool,
    enabled_scopes: &BTreeSet<SymbolScopeKey>,
) -> AppResult<BTreeMap<SymbolScopeKey, SymbolMeta>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
        WITH enabled(inst_id, inst_type) AS (
        "#,
    );
    push_enabled_scope_values(&mut query, enabled_scopes);
    query.push(
        r#"
        )
        SELECT c.inst_id, c.inst_type,
               GROUP_CONCAT(DISTINCT c.timeframe) AS timeframes,
               COUNT(*) AS candle_count
        FROM enabled e
        INNER JOIN candles c INDEXED BY idx_candles_query
          ON c.inst_id = e.inst_id
         AND c.inst_type = e.inst_type
        GROUP BY c.inst_id, c.inst_type
        ORDER BY c.inst_id
        "#,
    );
    let rows = query.build().fetch_all(db).await?;
    symbol_meta_from_rows(rows)
}

fn push_enabled_scope_values<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    enabled_scopes: &'a BTreeSet<SymbolScopeKey>,
) {
    query.push_values(enabled_scopes.iter(), |mut row, (inst_id, inst_type)| {
        row.push_bind(inst_id).push_bind(inst_type);
    });
}

fn watched_market_symbol_scopes(records: &[WatchedSymbolRecord]) -> BTreeSet<SymbolScopeKey> {
    let mut scopes = BTreeSet::new();
    for record in records {
        if record.sync_spot {
            scopes.insert((
                record.spot_inst_id.trim().to_uppercase(),
                "SPOT".to_string(),
            ));
        }
        if record.sync_swap {
            scopes.insert((
                record.swap_inst_id.trim().to_uppercase(),
                "SWAP".to_string(),
            ));
        }
    }
    scopes.retain(|(inst_id, inst_type)| !inst_id.is_empty() && !inst_type.is_empty());
    scopes
}

fn symbol_meta_from_rows(
    rows: Vec<sqlx::sqlite::SqliteRow>,
) -> AppResult<BTreeMap<SymbolScopeKey, SymbolMeta>> {
    let mut local_meta = BTreeMap::<SymbolScopeKey, SymbolMeta>::new();
    for row in rows {
        if let Some((scope, meta)) = symbol_meta_from_row(row)? {
            local_meta.insert(scope, meta);
        }
    }
    Ok(local_meta)
}

fn symbol_meta_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<Option<(SymbolScopeKey, SymbolMeta)>> {
    let inst_id = row.try_get::<String, _>("inst_id")?.trim().to_uppercase();
    let inst_type = row.try_get::<String, _>("inst_type")?.trim().to_uppercase();
    if inst_id.is_empty() || inst_type.is_empty() {
        return Ok(None);
    }
    let Some(raw_timeframes) = row.try_get::<Option<String>, _>("timeframes")? else {
        return Ok(None);
    };
    let timeframes = raw_timeframes
        .split(',')
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect::<Vec<_>>();
    let candle_count = row.try_get::<i64, _>("candle_count")?;
    if candle_count <= 0 {
        return Ok(None);
    }
    Ok(Some(((inst_id, inst_type), (timeframes, candle_count))))
}

pub(in crate::commands::local_api) async fn market_instruments(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type = param_string(req, "inst_type", "SPOT");
    let client = okx_client(state).await?;
    let instruments = client.get_instruments(&inst_type).await?;
    Ok(code_ok(Value::Array(instruments)))
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;

    #[tokio::test]
    async fn market_symbol_meta_prefers_sync_records_and_falls_back_by_scope() {
        let pool = test_pool().await;
        insert_sync_record(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 10).await;
        insert_sync_record(&pool, "BTC-USDT-SWAP", "SWAP", "5m", 20).await;
        insert_candles(&pool, "ETH-USDT-SWAP", "SWAP", "1m", 2).await;
        insert_candles(&pool, "ETH-USDT-SWAP", "SWAP", "1H", 1).await;
        insert_candles(&pool, "DOGE-USDT-SWAP", "SWAP", "1m", 99).await;

        let enabled = BTreeSet::from([
            ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
            ("ETH-USDT-SWAP".to_string(), "SWAP".to_string()),
        ]);
        let meta = load_market_symbol_meta(&pool, &enabled)
            .await
            .expect("load market symbol metadata");

        assert_eq!(
            meta[&("BTC-USDT-SWAP".to_string(), "SWAP".to_string())].1,
            30
        );
        assert_eq!(
            meta[&("ETH-USDT-SWAP".to_string(), "SWAP".to_string())].1,
            3
        );
        assert!(!meta.contains_key(&("DOGE-USDT-SWAP".to_string(), "SWAP".to_string())));
    }

    #[test]
    fn watched_market_symbol_scopes_collects_only_enabled_scopes() {
        let records = vec![WatchedSymbolRecord {
            symbol: "BTC-USDT".to_string(),
            base_ccy: "BTC".to_string(),
            spot_inst_id: "BTC-USDT".to_string(),
            swap_inst_id: "BTC-USDT-SWAP".to_string(),
            sync_spot: false,
            sync_swap: true,
            archive_all_history: false,
            sync_days: 90,
            sync_plans: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }];

        let scopes = watched_market_symbol_scopes(&records);

        assert_eq!(scopes.len(), 1);
        assert!(scopes.contains(&("BTC-USDT-SWAP".to_string(), "SWAP".to_string())));
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(
            r#"
            CREATE TABLE candles (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL,
              timeframe TEXT NOT NULL,
              timestamp INTEGER NOT NULL,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles");
        sqlx::query(
            "CREATE INDEX idx_candles_query ON candles(inst_id, inst_type, timeframe, timestamp)",
        )
        .execute(&pool)
        .await
        .expect("create candles query index");
        sqlx::query(
            r#"
            CREATE TABLE sync_records (
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL,
              timeframe TEXT NOT NULL,
              candle_count INTEGER NOT NULL DEFAULT 0,
              UNIQUE(inst_id, inst_type, timeframe)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create sync records");
        pool
    }

    async fn insert_sync_record(
        pool: &SqlitePool,
        inst_id: &str,
        inst_type: &str,
        timeframe: &str,
        candle_count: i64,
    ) {
        sqlx::query(
            "INSERT INTO sync_records (inst_id, inst_type, timeframe, candle_count) VALUES (?, ?, ?, ?)",
        )
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(candle_count)
        .execute(pool)
        .await
        .expect("insert sync record");
    }

    async fn insert_candles(
        pool: &SqlitePool,
        inst_id: &str,
        inst_type: &str,
        timeframe: &str,
        count: usize,
    ) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO candles (inst_id, inst_type, timeframe, timestamp) ",
        );
        query.push_values(0..count, |mut row, index| {
            row.push_bind(inst_id)
                .push_bind(inst_type)
                .push_bind(timeframe)
                .push_bind(1_700_000_000_000_i64 + index as i64);
        });
        query.build().execute(pool).await.expect("insert candles");
    }
}
