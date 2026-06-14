use super::entries::{add_i64_field, ensure_inventory_entry, ensure_inventory_market};
use super::*;
use crate::commands::local_api::market_ops::candle_coverage_metrics;
use crate::sync_record_summary::{
    normalize_sync_record_inst_type, push_allowed_market_scopes_cte, MarketScope,
};
use sqlx::SqlitePool;

#[derive(Clone, Debug)]
struct SyncRecordPayloadItem {
    inst_id: String,
    symbol: String,
    inst_type: String,
    timeframe: String,
    last_sync_time: Option<String>,
    oldest_timestamp: Option<i64>,
    newest_timestamp: Option<i64>,
    candle_count: i64,
    expected_candle_count: i64,
    gap_count: i64,
    coverage_ratio: f64,
    history_complete: bool,
    last_sync_mode: String,
}

impl SyncRecordPayloadItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> AppResult<Option<Self>> {
        let inst_id = row.try_get::<String, _>("inst_id")?.trim().to_uppercase();
        let Some(symbol) = normalize_symbol(&inst_id) else {
            return Ok(None);
        };
        let raw_inst_type = row.try_get::<String, _>("inst_type")?;
        let Some(inst_type) = normalize_sync_record_inst_type(&raw_inst_type) else {
            return Ok(None);
        };
        let timeframe = row.try_get::<String, _>("timeframe")?;
        let oldest_timestamp = row.try_get::<Option<i64>, _>("oldest_timestamp")?;
        let newest_timestamp = row.try_get::<Option<i64>, _>("newest_timestamp")?;
        let candle_count = row.try_get::<i64, _>("candle_count")?;
        let (expected_candle_count, gap_count, coverage_ratio) =
            candle_coverage_metrics(&timeframe, oldest_timestamp, newest_timestamp, candle_count);

        Ok(Some(Self {
            inst_id,
            symbol,
            inst_type,
            timeframe,
            last_sync_time: row.try_get::<Option<String>, _>("last_sync_time")?,
            oldest_timestamp,
            newest_timestamp,
            candle_count,
            expected_candle_count,
            gap_count,
            coverage_ratio,
            history_complete: row.try_get::<i64, _>("history_complete")? != 0,
            last_sync_mode: row.try_get::<String, _>("last_sync_mode")?,
        }))
    }

    fn to_value(&self) -> Value {
        json!({
            "inst_id": self.inst_id,
            "inst_type": self.inst_type,
            "timeframe": self.timeframe,
            "last_sync_time": self.last_sync_time,
            "oldest_timestamp": self.oldest_timestamp,
            "newest_timestamp": self.newest_timestamp,
            "oldest_time": ts_to_iso(self.oldest_timestamp),
            "newest_time": ts_to_iso(self.newest_timestamp),
            "candle_count": self.candle_count,
            "expected_candle_count": self.expected_candle_count,
            "gap_count": self.gap_count,
            "coverage_ratio": self.coverage_ratio,
            "history_complete": self.history_complete,
            "last_sync_mode": self.last_sync_mode,
        })
    }

    fn timeframe_inventory_value(&self, managed: bool, include_time_strings: bool) -> Value {
        let mut value = json!({
            "timeframe": self.timeframe,
            "candle_count": self.candle_count,
            "expected_candle_count": self.expected_candle_count,
            "gap_count": self.gap_count,
            "coverage_ratio": self.coverage_ratio,
            "managed": managed,
            "history_complete": self.history_complete,
            "last_sync_mode": self.last_sync_mode,
            "last_sync_time": self.last_sync_time,
            "oldest_timestamp": self.oldest_timestamp,
            "newest_timestamp": self.newest_timestamp,
        });
        if include_time_strings {
            let obj = value
                .as_object_mut()
                .expect("timeframe inventory value object");
            obj.insert("oldest_time".to_string(), ts_to_iso(self.oldest_timestamp));
            obj.insert("newest_time".to_string(), ts_to_iso(self.newest_timestamp));
        }
        value
    }
}

pub(in crate::commands::local_api) async fn market_sync_records(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let watched_only = param_bool(req, "watched_only", false);
    let watched_scopes = if watched_only {
        let watched_items = state.preferences.watched_symbols().await?;
        Some(scope_keys(&enabled_scopes_from_watched(&watched_items)))
    } else {
        None
    };
    Ok(code_ok(Value::Array(
        sync_record_payload_from_pool(&state.db, watched_scopes.as_ref()).await?,
    )))
}

pub(super) async fn apply_sync_record_rows(
    state: &AppState,
    entries: &mut BTreeMap<String, Value>,
    watched_symbols: &std::collections::BTreeSet<String>,
    managed_scope_keys: &std::collections::BTreeSet<(String, String)>,
    populate_storage_counts: bool,
    symbol_filter: Option<&str>,
) -> AppResult<()> {
    apply_sync_record_rows_from_pool(
        &state.db,
        entries,
        watched_symbols,
        managed_scope_keys,
        populate_storage_counts,
        symbol_filter,
    )
    .await
}

async fn apply_sync_record_rows_from_pool(
    pool: &SqlitePool,
    entries: &mut BTreeMap<String, Value>,
    watched_symbols: &std::collections::BTreeSet<String>,
    managed_scope_keys: &std::collections::BTreeSet<(String, String)>,
    populate_storage_counts: bool,
    symbol_filter: Option<&str>,
) -> AppResult<()> {
    let sync_rows = fetch_sync_record_rows(pool, symbol_filter).await?;
    apply_sync_record_rows_to_entries(
        sync_rows.into_iter(),
        entries,
        watched_symbols,
        managed_scope_keys,
        populate_storage_counts,
        false,
    )
}

fn apply_sync_record_rows_to_entries(
    sync_rows: impl Iterator<Item = sqlx::sqlite::SqliteRow>,
    entries: &mut BTreeMap<String, Value>,
    watched_symbols: &std::collections::BTreeSet<String>,
    managed_scope_keys: &std::collections::BTreeSet<(String, String)>,
    populate_storage_counts: bool,
    include_time_strings: bool,
) -> AppResult<()> {
    for row in sync_rows {
        let Some(item) = SyncRecordPayloadItem::from_row(&row)? else {
            continue;
        };
        let watched = watched_symbols.contains(&item.symbol);
        let managed = managed_scope_keys.contains(&scope_key(&item.inst_id, &item.inst_type));
        let entry = ensure_inventory_entry(entries, &item.symbol);

        if let Some(obj) = entry.as_object_mut() {
            add_i64_field(obj, "timeframe_record_count", 1);
            add_i64_field(obj, "candle_count", item.candle_count);
            if populate_storage_counts {
                if let Some(counts) = obj.get_mut("storage_counts").and_then(Value::as_object_mut) {
                    add_i64_field(counts, "candles", item.candle_count);
                    add_i64_field(counts, "sync_records", 1);
                }
            }
        }

        let market_obj =
            ensure_inventory_market(entry, &item.inst_type, &item.inst_id, managed, watched);
        add_i64_field(market_obj, "timeframe_count", 1);
        add_i64_field(market_obj, "candle_count", item.candle_count);
        add_i64_field(market_obj, "gap_count", item.gap_count);
        if item.history_complete {
            add_i64_field(market_obj, "history_complete_count", 1);
        }
        update_market_range(
            market_obj,
            item.oldest_timestamp,
            item.newest_timestamp,
            item.last_sync_time.as_deref(),
            include_time_strings,
        );
        market_obj
            .entry("timeframes".to_string())
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .expect("timeframes array")
            .push(item.timeframe_inventory_value(managed, include_time_strings));
    }
    Ok(())
}

async fn sync_record_payload_from_pool(
    pool: &SqlitePool,
    scope_filter: Option<&std::collections::BTreeSet<MarketScope>>,
) -> AppResult<Vec<Value>> {
    let sync_rows = match scope_filter {
        Some(scopes) => fetch_sync_record_rows_for_scopes(pool, scopes).await?,
        None => fetch_all_sync_record_rows(pool).await?,
    };
    let mut payload = Vec::new();
    for row in sync_rows {
        if let Some(item) = SyncRecordPayloadItem::from_row(&row)? {
            payload.push(item.to_value());
        }
    }
    Ok(payload)
}

async fn fetch_sync_record_rows(
    pool: &SqlitePool,
    symbol_filter: Option<&str>,
) -> AppResult<Vec<sqlx::sqlite::SqliteRow>> {
    let Some(symbol) = symbol_filter else {
        return fetch_all_sync_record_rows(pool).await;
    };
    let Some((spot, swap)) = symbol_sync_record_inst_ids(symbol) else {
        return Ok(Vec::new());
    };
    Ok(sqlx::query(
        r#"
        SELECT inst_id, inst_type, timeframe, last_sync_time,
               oldest_timestamp, newest_timestamp, candle_count,
               history_complete, last_sync_mode
        FROM sync_records
        WHERE inst_id IN (?, ?)
        ORDER BY inst_id, timeframe
        "#,
    )
    .bind(spot)
    .bind(swap)
    .fetch_all(pool)
    .await?)
}

async fn fetch_all_sync_record_rows(pool: &SqlitePool) -> AppResult<Vec<sqlx::sqlite::SqliteRow>> {
    Ok(sqlx::query(
        r#"
        SELECT inst_id, inst_type, timeframe, last_sync_time,
               oldest_timestamp, newest_timestamp, candle_count,
               history_complete, last_sync_mode
        FROM sync_records
        ORDER BY inst_id, timeframe
        "#,
    )
    .fetch_all(pool)
    .await?)
}

async fn fetch_sync_record_rows_for_scopes(
    pool: &SqlitePool,
    scopes: &std::collections::BTreeSet<MarketScope>,
) -> AppResult<Vec<sqlx::sqlite::SqliteRow>> {
    if scopes.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new("");
    push_allowed_market_scopes_cte(&mut query, scopes);
    query.push(
        r#"
        SELECT r.inst_id, r.inst_type, r.timeframe, r.last_sync_time,
               r.oldest_timestamp, r.newest_timestamp, r.candle_count,
               r.history_complete, r.last_sync_mode
        FROM allowed a
        INNER JOIN sync_records r
          ON r.inst_id = a.inst_id
         AND r.inst_type = a.inst_type
        ORDER BY r.inst_id, r.timeframe
        "#,
    );
    Ok(query.build().fetch_all(pool).await?)
}

fn symbol_sync_record_inst_ids(symbol: &str) -> Option<(String, String)> {
    let (_normalized, spot, swap, _base) = symbol_parts(symbol)?;
    Some((spot, swap))
}

fn update_market_range(
    market_obj: &mut serde_json::Map<String, Value>,
    oldest_timestamp: Option<i64>,
    newest_timestamp: Option<i64>,
    last_sync_time: Option<&str>,
    include_time_strings: bool,
) {
    if let Some(timestamp) = oldest_timestamp {
        let current = market_obj.get("oldest_timestamp").and_then(Value::as_i64);
        if current.map_or(true, |value| timestamp < value) {
            market_obj.insert(
                "oldest_timestamp".to_string(),
                Value::Number(timestamp.into()),
            );
            if include_time_strings {
                market_obj.insert("oldest_time".to_string(), ts_to_iso(Some(timestamp)));
            }
        }
    }
    if let Some(timestamp) = newest_timestamp {
        let current = market_obj.get("newest_timestamp").and_then(Value::as_i64);
        if current.map_or(true, |value| timestamp > value) {
            market_obj.insert(
                "newest_timestamp".to_string(),
                Value::Number(timestamp.into()),
            );
            if include_time_strings {
                market_obj.insert("newest_time".to_string(), ts_to_iso(Some(timestamp)));
            }
        }
    }
    let Some(last_sync_time) = last_sync_time.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    let current = market_obj
        .get("last_sync_time")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if current.is_empty() || last_sync_time > current {
        market_obj.insert("last_sync_time".to_string(), json!(last_sync_time));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, QueryBuilder, Sqlite};

    #[test]
    fn coverage_metrics_detects_internal_candle_gaps() {
        let (expected, gaps, ratio) = candle_coverage_metrics("1m", Some(0), Some(4 * 60_000), 4);

        assert_eq!(expected, 5);
        assert_eq!(gaps, 1);
        assert!((ratio - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn coverage_metrics_handles_empty_ranges() {
        let (expected, gaps, ratio) = candle_coverage_metrics("1H", None, None, 0);

        assert_eq!(expected, 0);
        assert_eq!(gaps, 0);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn market_range_uses_earliest_oldest_latest_newest_and_latest_sync_time() {
        let mut market = serde_json::Map::new();

        update_market_range(
            &mut market,
            Some(120_000),
            Some(180_000),
            Some("2026-05-01T00:00:00Z"),
            true,
        );
        update_market_range(
            &mut market,
            Some(60_000),
            Some(240_000),
            Some("2026-05-02T00:00:00Z"),
            true,
        );
        update_market_range(
            &mut market,
            Some(90_000),
            Some(210_000),
            Some("2026-04-30T00:00:00Z"),
            true,
        );

        assert_eq!(
            market.get("oldest_timestamp").and_then(Value::as_i64),
            Some(60_000)
        );
        assert_eq!(
            market.get("newest_timestamp").and_then(Value::as_i64),
            Some(240_000)
        );
        assert_eq!(
            market.get("last_sync_time").and_then(Value::as_str),
            Some("2026-05-02T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn filtered_sync_record_rows_populate_only_target_symbol_inventory() {
        let pool = memory_pool().await;
        create_sync_records_table(&pool).await;
        insert_sync_records(&pool, 20, 3).await;
        let target = "PERF-0007-USDT";
        let watched_symbols = std::collections::BTreeSet::from([target.to_string()]);
        let managed_scope_keys = std::collections::BTreeSet::from([
            (target.to_string(), "SPOT".to_string()),
            (format!("{target}-SWAP"), "SWAP".to_string()),
        ]);

        let mut filtered_entries = BTreeMap::new();
        apply_sync_record_rows_from_pool(
            &pool,
            &mut filtered_entries,
            &watched_symbols,
            &managed_scope_keys,
            true,
            Some(target),
        )
        .await
        .expect("filtered sync record rows");

        let entry = filtered_entries
            .get(target)
            .expect("target inventory entry");
        assert_eq!(filtered_entries.len(), 1);
        assert_eq!(entry["timeframe_record_count"], 6);
        assert_eq!(entry["candle_count"], 6006);
        assert_eq!(entry["storage_counts"]["candles"], 6006);
        assert_eq!(entry["storage_counts"]["sync_records"], 6);

        let markets = entry["markets"].as_object().expect("markets object");
        for (inst_type, inst_id) in [
            ("SPOT", target.to_string()),
            ("SWAP", format!("{target}-SWAP")),
        ] {
            let market = markets.get(inst_type).expect("market entry");
            assert_eq!(market["inst_id"], inst_id);
            assert_eq!(market["inst_type"], inst_type);
            assert_eq!(market["managed"], true);
            assert_eq!(market["watched"], true);
            assert_eq!(market["timeframe_count"], 3);
            assert_eq!(market["candle_count"], 3003);
            assert_eq!(market["gap_count"], 0);
            assert_eq!(market["history_complete_count"], 2);
            assert_eq!(market["oldest_timestamp"], 1_700_000_000_000_i64);
            assert_eq!(market["newest_timestamp"], 1_700_000_120_000_i64);
            assert_eq!(market["last_sync_time"], "2026-05-01T00:00:00Z");
            let timeframes = market["timeframes"].as_array().expect("timeframes array");
            assert_eq!(timeframes.len(), 3);
            assert!(
                timeframes
                    .iter()
                    .all(|timeframe| timeframe.get("oldest_time").is_none()),
                "inventory projection should omit redundant timeframe ISO strings"
            );
        }
    }

    #[tokio::test]
    async fn watched_sync_record_payload_returns_scoped_records_directly() {
        let pool = memory_pool().await;
        create_sync_records_table(&pool).await;
        insert_sync_records(&pool, 20, 3).await;
        let target = "PERF-0007-USDT";
        let watched_scopes = std::collections::BTreeSet::from([
            (target.to_string(), "SPOT".to_string()),
            (format!("{target}-SWAP"), "SWAP".to_string()),
        ]);

        let direct = sync_record_payload_from_pool(&pool, Some(&watched_scopes))
            .await
            .expect("direct watched sync records");

        assert_eq!(
            direct,
            vec![
                expected_sync_record_value(target, "SPOT", 0),
                expected_sync_record_value(target, "SPOT", 1),
                expected_sync_record_value(target, "SPOT", 2),
                expected_sync_record_value(&format!("{target}-SWAP"), "SWAP", 0),
                expected_sync_record_value(&format!("{target}-SWAP"), "SWAP", 1),
                expected_sync_record_value(&format!("{target}-SWAP"), "SWAP", 2),
            ]
        );
    }

    fn expected_sync_record_value(inst_id: &str, inst_type: &str, timeframe_index: usize) -> Value {
        let timeframe = format!("{}m", timeframe_index + 1);
        let newest_timestamp = 1_700_000_000_000_i64 + timeframe_index as i64 * 60_000;
        let candle_count = 1_000_i64 + timeframe_index as i64;
        json!({
            "inst_id": inst_id,
            "inst_type": inst_type,
            "timeframe": timeframe,
            "last_sync_time": "2026-05-01T00:00:00Z",
            "oldest_timestamp": 1_700_000_000_000_i64,
            "newest_timestamp": newest_timestamp,
            "oldest_time": ts_to_iso(Some(1_700_000_000_000_i64)),
            "newest_time": ts_to_iso(Some(newest_timestamp)),
            "candle_count": candle_count,
            "expected_candle_count": candle_count,
            "gap_count": 0,
            "coverage_ratio": 1.0,
            "history_complete": timeframe_index % 2 == 0,
            "last_sync_mode": "window",
        })
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
                last_sync_time TIMESTAMP,
                oldest_timestamp INTEGER,
                newest_timestamp INTEGER,
                candle_count INTEGER DEFAULT 0,
                history_complete INTEGER NOT NULL DEFAULT 0,
                last_sync_mode TEXT NOT NULL DEFAULT 'window',
                UNIQUE(inst_id, inst_type, timeframe)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create sync_records table");
    }

    async fn insert_sync_records(pool: &SqlitePool, symbol_count: usize, timeframe_count: usize) {
        const CHUNK_SIZE: usize = 500;
        let total_rows = symbol_count * 2 * timeframe_count;
        let mut inserted = 0usize;
        let mut tx = pool.begin().await.expect("begin tx");
        while inserted < total_rows {
            let chunk_end = (inserted + CHUNK_SIZE).min(total_rows);
            let mut query = QueryBuilder::<Sqlite>::new(
                r#"
                INSERT INTO sync_records (
                    inst_id, inst_type, timeframe, last_sync_time,
                    oldest_timestamp, newest_timestamp, candle_count,
                    history_complete, last_sync_mode
                )
                "#,
            );
            query.push_values(inserted..chunk_end, |mut row, index| {
                let timeframe_index = index % timeframe_count;
                let market_index = (index / timeframe_count) % 2;
                let symbol_index = index / (timeframe_count * 2);
                let base = format!("PERF-{symbol_index:04}-USDT");
                let (inst_id, inst_type) = if market_index == 0 {
                    (base, "SPOT".to_string())
                } else {
                    (format!("{base}-SWAP"), "SWAP".to_string())
                };
                let newest = 1_700_000_000_000_i64 + timeframe_index as i64 * 60_000;
                row.push_bind(inst_id)
                    .push_bind(inst_type)
                    .push_bind(format!("{}m", timeframe_index + 1))
                    .push_bind("2026-05-01T00:00:00Z")
                    .push_bind(1_700_000_000_000_i64)
                    .push_bind(newest)
                    .push_bind(1_000_i64 + timeframe_index as i64)
                    .push_bind(if timeframe_index % 2 == 0 { 1 } else { 0 })
                    .push_bind("window");
            });
            query
                .build()
                .execute(&mut *tx)
                .await
                .expect("insert sync_records");
            inserted = chunk_end;
        }
        tx.commit().await.expect("commit tx");
    }
}
