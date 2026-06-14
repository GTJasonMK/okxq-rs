use std::{collections::BTreeMap, sync::Arc};

use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tokio::sync::RwLock;

use crate::instrument::infer_okx_inst_type;

use super::{bar::SecondBar, types::TickCollectorStatus};

const TICK_COLLECTOR_INSERT_CHUNK: usize = 500;

pub(super) struct TradeRecord {
    pub(super) inst_id: String,
    pub(super) side: String,
    pub(super) trade_id: String,
    pub(super) price: f64,
    pub(super) size: f64,
    pub(super) ts: i64,
}

struct BarInsertRow {
    inst_id: String,
    ts: i64,
    payload_json: String,
}

struct TradeInsertRow {
    inst_id: String,
    inst_type: &'static str,
    trade_id: String,
    payload_json: String,
    ts: i64,
}

/// 将所有已完成秒桶的 OHLCV 数据写入 feature_bars_1s。
pub(super) async fn flush_bars(
    db: &SqlitePool,
    bars: &mut BTreeMap<(String, i64), SecondBar>,
    flush_through_second: i64,
    bar_count: &mut i64,
    status: &Arc<RwLock<TickCollectorStatus>>,
) {
    if flush_through_second <= 0 || bars.is_empty() {
        return;
    }

    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let keys = bars
        .keys()
        .filter(|(_, second)| *second <= flush_through_second)
        .cloned()
        .collect::<Vec<_>>();

    let mut rows = Vec::with_capacity(keys.len());
    for (inst_id, second_bucket) in keys {
        let Some(bar) = bars.remove(&(inst_id.clone(), second_bucket)) else {
            continue;
        };
        let Some(payload) = bar.to_payload_json() else {
            continue;
        };

        let ts = second_bucket * 1000; // 转回毫秒
        rows.push(BarInsertRow {
            inst_id,
            ts,
            payload_json: payload.to_string(),
        });
    }

    if !rows.is_empty() {
        match insert_bar_rows(db, &rows, now).await {
            Ok(()) => *bar_count += rows.len() as i64,
            Err(e) => push_error(status, format!("批量写入秒柱失败 {} 行: {e}", rows.len())).await,
        }
    }

    let mut s = status.write().await;
    s.total_bars_written = *bar_count;
}

/// 批量写入逐笔成交到 market_recent_trades。
pub(super) async fn flush_trade_buffer(db: &SqlitePool, buffer: &mut Vec<TradeRecord>) {
    if buffer.is_empty() {
        return;
    }

    let now = chrono::Utc::now().to_rfc3339();

    let mut rows = Vec::with_capacity(buffer.len());
    for record in buffer.drain(..) {
        if !is_valid_trade_record(&record) {
            continue;
        }
        let payload = serde_json::json!({
            "inst_id": &record.inst_id,
            "side": &record.side,
            "trade_id": &record.trade_id,
            "price": record.price,
            "size": record.size,
            "ts": record.ts,
        })
        .to_string();

        let inst_type = infer_okx_inst_type(&record.inst_id);
        rows.push(TradeInsertRow {
            inst_id: record.inst_id,
            inst_type,
            trade_id: record.trade_id,
            payload_json: payload,
            ts: record.ts,
        });
    }

    if !rows.is_empty() {
        let _ = insert_trade_rows(db, &rows, &now).await;
    }
}

async fn insert_bar_rows(
    db: &SqlitePool,
    rows: &[BarInsertRow],
    now: f64,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for chunk in rows.chunks(TICK_COLLECTOR_INSERT_CHUNK) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT OR REPLACE INTO feature_bars_1s (inst_id, ts, payload_json, created_at) ",
        );
        query.push_values(chunk, |mut row_builder, row| {
            row_builder
                .push_bind(&row.inst_id)
                .push_bind(row.ts)
                .push_bind(&row.payload_json)
                .push_bind(now);
        });
        query.build().execute(&mut *tx).await?;
    }
    tx.commit().await
}

async fn insert_trade_rows(
    db: &SqlitePool,
    rows: &[TradeInsertRow],
    now: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for chunk in rows.chunks(TICK_COLLECTOR_INSERT_CHUNK) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT OR IGNORE INTO market_recent_trades (inst_id, inst_type, trade_id, payload_json, ts, created_at) ",
        );
        query.push_values(chunk, |mut row_builder, row| {
            row_builder
                .push_bind(&row.inst_id)
                .push_bind(row.inst_type)
                .push_bind(&row.trade_id)
                .push_bind(&row.payload_json)
                .push_bind(row.ts)
                .push_bind(now);
        });
        query.build().execute(&mut *tx).await?;
    }
    tx.commit().await
}

fn is_valid_trade_record(record: &TradeRecord) -> bool {
    let side = record.side.trim().to_ascii_lowercase();
    !record.inst_id.trim().is_empty()
        && !record.trade_id.trim().is_empty()
        && matches!(side.as_str(), "buy" | "sell")
        && record.ts > 0
        && record.price.is_finite()
        && record.price > 0.0
        && record.size.is_finite()
        && record.size > 0.0
}

async fn push_error(status: &Arc<RwLock<TickCollectorStatus>>, message: String) {
    let mut s = status.write().await;
    if s.errors.len() > 20 {
        s.errors.remove(0);
    }
    s.errors.push(message);
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use sqlx::{sqlite::SqlitePoolOptions, Row};
    use tokio::sync::RwLock;

    use super::{flush_bars, flush_trade_buffer, SecondBar, TickCollectorStatus, TradeRecord};

    #[tokio::test]
    async fn flush_trade_buffer_does_not_persist_invalid_price_or_size() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        create_recent_trades_table(&pool).await;
        let mut buffer = vec![
            TradeRecord {
                inst_id: "BTC-USDT-SWAP".to_string(),
                side: "buy".to_string(),
                trade_id: "bad-price".to_string(),
                price: 0.0,
                size: 1.0,
                ts: 1700000000000,
            },
            TradeRecord {
                inst_id: "BTC-USDT-SWAP".to_string(),
                side: "buy".to_string(),
                trade_id: "bad-size".to_string(),
                price: 100.0,
                size: f64::NAN,
                ts: 1700000000001,
            },
        ];

        flush_trade_buffer(&pool, &mut buffer).await;

        let count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM market_recent_trades")
            .fetch_one(&pool)
            .await
            .expect("count")
            .try_get("count")
            .expect("count value");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn flush_trade_buffer_does_not_persist_missing_identity_or_invalid_timestamp() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        create_recent_trades_table(&pool).await;
        let mut buffer = vec![
            TradeRecord {
                inst_id: "BTC-USDT-SWAP".to_string(),
                side: "buy".to_string(),
                trade_id: "".to_string(),
                price: 100.0,
                size: 1.0,
                ts: 1700000000000,
            },
            TradeRecord {
                inst_id: "BTC-USDT-SWAP".to_string(),
                side: "hold".to_string(),
                trade_id: "bad-side".to_string(),
                price: 100.0,
                size: 1.0,
                ts: 1700000000001,
            },
            TradeRecord {
                inst_id: "BTC-USDT-SWAP".to_string(),
                side: "sell".to_string(),
                trade_id: "bad-ts".to_string(),
                price: 100.0,
                size: 1.0,
                ts: 0,
            },
        ];

        flush_trade_buffer(&pool, &mut buffer).await;

        let count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM market_recent_trades")
            .fetch_one(&pool)
            .await
            .expect("count")
            .try_get("count")
            .expect("count value");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn flush_bars_writes_ready_trade_bars_and_keeps_future_buckets() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        create_feature_bars_table(&pool).await;

        let mut ready_bar = SecondBar::default();
        ready_bar.ingest_book(100.0, 101.0);
        ready_bar.ingest_trade(100.5, 2.0);
        let mut quote_only_bar = SecondBar::default();
        quote_only_bar.ingest_book(200.0, 201.0);
        let mut future_bar = SecondBar::default();
        future_bar.ingest_trade(300.0, 3.0);

        let mut bars = BTreeMap::new();
        bars.insert(("BTC-USDT-SWAP".to_string(), 100), ready_bar);
        bars.insert(("ETH-USDT-SWAP".to_string(), 100), quote_only_bar);
        bars.insert(("SOL-USDT-SWAP".to_string(), 101), future_bar);

        let status = Arc::new(RwLock::new(TickCollectorStatus::default()));
        let mut bar_count = 0;
        flush_bars(&pool, &mut bars, 100, &mut bar_count, &status).await;

        let persisted_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM feature_bars_1s")
            .fetch_one(&pool)
            .await
            .expect("count feature bars")
            .try_get("count")
            .expect("feature bar count value");
        assert_eq!(persisted_count, 1);
        assert_eq!(bar_count, 1);
        assert_eq!(status.read().await.total_bars_written, 1);
        assert_eq!(bars.len(), 1);
        assert!(bars.contains_key(&("SOL-USDT-SWAP".to_string(), 101)));
    }

    async fn create_feature_bars_table(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE feature_bars_1s (
              inst_id TEXT NOT NULL,
              ts INTEGER NOT NULL,
              payload_json TEXT NOT NULL DEFAULT '{}',
              created_at REAL NOT NULL,
              PRIMARY KEY(inst_id, ts)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create feature bars table");
    }

    async fn create_recent_trades_table(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE market_recent_trades (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SPOT',
              trade_id TEXT NOT NULL DEFAULT '',
              payload_json TEXT NOT NULL DEFAULT '{}',
              ts INTEGER NOT NULL,
              created_at TEXT NOT NULL,
              UNIQUE(inst_id, inst_type, trade_id)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create table");
        sqlx::query(
            "CREATE INDEX idx_market_trades_inst_time ON market_recent_trades(inst_id, ts)",
        )
        .execute(pool)
        .await
        .expect("create inst time index");
        sqlx::query(
            "CREATE INDEX idx_market_trades_type_time ON market_recent_trades(inst_type, ts)",
        )
        .execute(pool)
        .await
        .expect("create type time index");
    }
}
