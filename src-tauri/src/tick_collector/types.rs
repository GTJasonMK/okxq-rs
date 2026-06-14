use serde::Serialize;

/// 从 WebSocket 转发过来的行情事件。
#[derive(Clone, Debug)]
pub enum TickEvent {
    Trade {
        inst_id: String,
        price: f64,
        size: f64,
        side: String,
        trade_id: String,
        ts: i64,
    },
    OrderBookMid {
        inst_id: String,
        bid: f64,
        ask: f64,
        ts: i64,
    },
}

/// TickCollector 运行状态。
#[derive(Clone, Debug, Serialize)]
pub struct TickCollectorStatus {
    pub running: bool,
    pub active_symbols: Vec<String>,
    pub book_channel: String,
    pub total_trades_received: i64,
    pub total_bars_written: i64,
    pub last_trade_ts: i64,
    pub errors: Vec<String>,
}

impl Default for TickCollectorStatus {
    fn default() -> Self {
        Self {
            running: false,
            active_symbols: Vec::new(),
            book_channel: "books5".to_string(),
            total_trades_received: 0,
            total_bars_written: 0,
            last_trade_ts: 0,
            errors: Vec::new(),
        }
    }
}
