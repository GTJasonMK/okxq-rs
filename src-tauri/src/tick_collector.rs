//! 秒级数据采集引擎 — 从 WebSocket 实时流中采集逐笔成交和订单簿，聚合成秒级 K 线。
//!
//! 数据流：OKX WS → realtime.rs emit_trade/emit_orderbook → mpsc channel → TickCollector
//!   → market_recent_trades (raw trades) + feature_bars_1s (aggregated OHLCV bars).
//!
//! 生命周期由 TickCollectorManager 管理，支持启动/停止/状态查询。

mod bar;
mod runtime;
mod storage;
mod symbols;
mod types;

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::config::TrendResearchConfig;

use self::{
    runtime::run_collector,
    symbols::{collection_symbol_filter, normalize_book_channel, normalize_collection_symbols},
};

pub use self::types::{TickCollectorStatus, TickEvent};

/// 采集器管理器 — 支持启动/停止/状态查询。
pub struct TickCollectorManager {
    status: Arc<RwLock<TickCollectorStatus>>,
    stop_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl TickCollectorManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(TickCollectorStatus::default())),
            stop_tx: Mutex::new(None),
        }
    }

    pub async fn status(&self) -> TickCollectorStatus {
        self.status.read().await.clone()
    }

    /// 启动采集器后台任务，返回 mpsc sender 供 realtime 模块推送数据。
    pub async fn start(
        &self,
        db: SqlitePool,
        config: TrendResearchConfig,
    ) -> Result<mpsc::UnboundedSender<TickEvent>, String> {
        {
            let status = self.status.read().await;
            if status.running {
                return Err("秒级采集器已在运行".to_string());
            }
        }

        let active_symbols = normalize_collection_symbols(&config.whitelist)?;
        let symbol_filter = collection_symbol_filter(&active_symbols);
        let book_channel = normalize_book_channel(&config.book_channel)?;

        let (tx, rx) = mpsc::unbounded_channel::<TickEvent>();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();

        {
            let mut guard = self.stop_tx.lock().await;
            *guard = Some(stop_tx);
        }

        {
            let mut status = self.status.write().await;
            *status = TickCollectorStatus {
                running: true,
                active_symbols,
                book_channel,
                ..TickCollectorStatus::default()
            };
        }

        let status_ref = self.status.clone();
        tokio::spawn(async move {
            run_collector(rx, stop_rx, db, symbol_filter, status_ref).await;
        });

        Ok(tx)
    }

    pub async fn stop(&self) -> TickCollectorStatus {
        if let Some(sender) = self.stop_tx.lock().await.take() {
            let _ = sender.send(());
        }
        let mut status = self.status.write().await;
        status.running = false;
        status.clone()
    }
}
