use std::collections::BTreeSet;

use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite, SqlitePool};

use crate::{config::WatchedSymbolRecord, error::AppResult};

pub(crate) type MarketScope = (String, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SyncRecordMarketAggregate {
    pub(crate) inst_id: String,
    pub(crate) inst_type: String,
    pub(crate) candle_count: i64,
    pub(crate) timeframe_count: i64,
}

pub(crate) fn watched_market_scopes(records: &[WatchedSymbolRecord]) -> BTreeSet<MarketScope> {
    let mut scopes = BTreeSet::new();
    for item in records {
        if item.sync_spot {
            scopes.insert((item.spot_inst_id.trim().to_uppercase(), "SPOT".to_string()));
        }
        if item.sync_swap {
            scopes.insert((item.swap_inst_id.trim().to_uppercase(), "SWAP".to_string()));
        }
    }
    scopes
}

pub(crate) fn normalize_sync_record_inst_type(raw_inst_type: &str) -> Option<String> {
    let inst_type = raw_inst_type.trim().to_uppercase();
    if !inst_type.is_empty() {
        Some(inst_type)
    } else {
        None
    }
}

pub(crate) async fn load_all_sync_record_market_aggregates(
    pool: &SqlitePool,
) -> AppResult<Vec<SyncRecordMarketAggregate>> {
    let rows = sqlx::query(
        r#"
        SELECT inst_id, inst_type,
               COALESCE(SUM(CASE WHEN candle_count > 0 THEN candle_count ELSE 0 END), 0) AS candle_count,
               COUNT(*) AS timeframe_count
        FROM sync_records
        WHERE TRIM(COALESCE(inst_id, '')) <> ''
        GROUP BY inst_id, inst_type
        ORDER BY inst_id, inst_type
        "#,
    )
    .fetch_all(pool)
    .await?;
    sync_record_market_aggregates_from_rows(rows)
}

pub(crate) async fn load_sync_record_market_aggregates_for_scopes(
    pool: &SqlitePool,
    scopes: &BTreeSet<MarketScope>,
) -> AppResult<Vec<SyncRecordMarketAggregate>> {
    if scopes.is_empty() {
        return Ok(Vec::new());
    }

    let mut query = QueryBuilder::<Sqlite>::new("");
    push_allowed_market_scopes_cte(&mut query, scopes);
    query.push(
        r#"
        SELECT r.inst_id, r.inst_type,
               COALESCE(SUM(CASE WHEN r.candle_count > 0 THEN r.candle_count ELSE 0 END), 0) AS candle_count,
               COUNT(*) AS timeframe_count
        FROM allowed a
        INNER JOIN sync_records r
          ON r.inst_id = a.inst_id
         AND r.inst_type = a.inst_type
        WHERE TRIM(COALESCE(r.inst_id, '')) <> ''
        GROUP BY r.inst_id, r.inst_type
        ORDER BY r.inst_id, r.inst_type
        "#,
    );
    let rows = query.build().fetch_all(pool).await?;
    sync_record_market_aggregates_from_rows(rows)
}

pub(crate) fn push_allowed_market_scopes_cte<'args>(
    query: &mut QueryBuilder<'args, Sqlite>,
    scopes: &'args BTreeSet<MarketScope>,
) {
    query.push(
        r#"
        WITH allowed(inst_id, inst_type) AS (
        "#,
    );
    query.push_values(scopes.iter(), |mut row, (inst_id, inst_type)| {
        row.push_bind(inst_id).push_bind(inst_type);
    });
    query.push(
        r#"
        )
        "#,
    );
}

fn sync_record_market_aggregates_from_rows(
    rows: Vec<SqliteRow>,
) -> AppResult<Vec<SyncRecordMarketAggregate>> {
    let mut aggregates = Vec::new();
    for row in rows {
        if let Some(aggregate) = sync_record_market_aggregate_from_row(row)? {
            aggregates.push(aggregate);
        }
    }
    Ok(aggregates)
}

fn sync_record_market_aggregate_from_row(
    row: SqliteRow,
) -> AppResult<Option<SyncRecordMarketAggregate>> {
    let inst_id = row.try_get::<String, _>("inst_id")?;
    let inst_id = inst_id.trim().to_uppercase();
    if inst_id.is_empty() {
        return Ok(None);
    }
    let raw_inst_type = row.try_get::<String, _>("inst_type")?;
    let Some(inst_type) = normalize_sync_record_inst_type(&raw_inst_type) else {
        return Ok(None);
    };
    Ok(Some(SyncRecordMarketAggregate {
        inst_type,
        inst_id,
        candle_count: row.try_get::<i64, _>("candle_count")?.max(0),
        timeframe_count: row.try_get::<i64, _>("timeframe_count")?.max(0),
    }))
}
