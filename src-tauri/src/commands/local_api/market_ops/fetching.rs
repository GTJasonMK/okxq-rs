mod full;
mod incremental;
mod paging;
mod window;

use sqlx::SqlitePool;

use crate::okx::OkxPublicClient;

use super::*;

#[derive(Clone, Copy)]
pub(in crate::commands::local_api::market_ops) struct CandleFetchContext<'a> {
    pub(in crate::commands::local_api::market_ops) pool: &'a SqlitePool,
    pub(in crate::commands::local_api::market_ops) client: &'a OkxPublicClient,
    pub(in crate::commands::local_api::market_ops) inst_id: &'a str,
    pub(in crate::commands::local_api::market_ops) inst_type: &'a str,
    pub(in crate::commands::local_api::market_ops) timeframe: &'a str,
    pub(in crate::commands::local_api::market_ops) cancel_guard: Option<&'a SyncCancelGuard>,
    pub(in crate::commands::local_api::market_ops) progress: &'a SyncProgressReporter,
}

pub(in crate::commands::local_api::market_ops) struct CandleFetchRange {
    pub(in crate::commands::local_api::market_ops) start_ts: i64,
    pub(in crate::commands::local_api::market_ops) end_ts: i64,
    pub(in crate::commands::local_api::market_ops) completion_message: String,
}

pub(super) use self::full::fetch_full_candles;
pub(super) use self::incremental::fetch_incremental_candles;
pub(super) use self::window::{fetch_range_candles, fetch_window_candles};
