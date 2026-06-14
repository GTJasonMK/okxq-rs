use std::{collections::BTreeMap, collections::BTreeSet, sync::Arc};

use sqlx::SqlitePool;
use tokio::sync::{mpsc, RwLock};

use super::{
    bar::SecondBar,
    storage::{flush_bars, flush_trade_buffer, TradeRecord},
    types::{TickCollectorStatus, TickEvent},
};

/// 采集器主循环。
pub(super) async fn run_collector(
    mut rx: mpsc::UnboundedReceiver<TickEvent>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    db: SqlitePool,
    symbol_filter: BTreeSet<String>,
    status: Arc<RwLock<TickCollectorStatus>>,
) {
    // 每个币种每一秒独立成桶，避免多币种和盘口/成交乱序污染同一个秒柱。
    let mut bars: BTreeMap<(String, i64), SecondBar> = BTreeMap::new();
    let mut trade_buffer: Vec<TradeRecord> = Vec::new();
    let mut flush_tick = tokio::time::interval(std::time::Duration::from_millis(200));
    let mut bar_count: i64 = 0;

    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                flush_bars(&db, &mut bars, i64::MAX, &mut bar_count, &status).await;
                flush_trade_buffer(&db, &mut trade_buffer).await;
                mark_stopped(&status).await;
                return;
            }
            event = rx.recv() => {
                match event {
                    Some(TickEvent::Trade { inst_id, price, size, side, trade_id, ts }) => {
                        if !symbol_filter.contains(&inst_id) {
                            continue;
                        }

                        let second_bucket = ts / 1000;
                        flush_bars(&db, &mut bars, second_bucket - 1, &mut bar_count, &status).await;
                        bars
                            .entry((inst_id.clone(), second_bucket))
                            .or_default()
                            .ingest_trade(price, size);

                        trade_buffer.push(TradeRecord {
                            inst_id,
                            side,
                            trade_id,
                            price,
                            size,
                            ts,
                        });

                        if trade_buffer.len() >= 200 {
                            flush_trade_buffer(&db, &mut trade_buffer).await;
                        }

                        let mut s = status.write().await;
                        s.total_trades_received += 1;
                        s.last_trade_ts = ts;
                    }
                    Some(TickEvent::OrderBookMid { inst_id, bid, ask, ts }) => {
                        if !symbol_filter.contains(&inst_id) {
                            continue;
                        }

                        let second_bucket = ts / 1000;
                        flush_bars(&db, &mut bars, second_bucket - 1, &mut bar_count, &status).await;
                        bars
                            .entry((inst_id, second_bucket))
                            .or_default()
                            .ingest_book(bid, ask);
                    }
                    None => {
                        flush_bars(&db, &mut bars, i64::MAX, &mut bar_count, &status).await;
                        flush_trade_buffer(&db, &mut trade_buffer).await;
                        mark_stopped(&status).await;
                        return;
                    }
                }
            }
            _ = flush_tick.tick() => {
                if !trade_buffer.is_empty() {
                    flush_trade_buffer(&db, &mut trade_buffer).await;
                }
                let flush_through = chrono::Utc::now().timestamp_millis() / 1000 - 1;
                flush_bars(&db, &mut bars, flush_through, &mut bar_count, &status).await;
            }
        }
    }
}

async fn mark_stopped(status: &Arc<RwLock<TickCollectorStatus>>) {
    let mut s = status.write().await;
    s.running = false;
}
