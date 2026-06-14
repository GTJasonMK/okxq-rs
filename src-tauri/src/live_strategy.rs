use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use tokio::sync::{oneshot, Mutex, Notify, RwLock};

mod arrival;
mod client;
pub(crate) mod decision;
mod logs;
mod order_sync;
mod payload;
mod runtime;
mod runtime_helpers;
mod single_eval;
pub(crate) mod state_context;
mod storage;
mod types;

pub use self::client::build_live_strategy_client;
pub(crate) use self::client::build_live_strategy_private_client;
pub(crate) use self::order_sync::{
    persist_private_algo_order_event, persist_private_fill_event, persist_private_order_event,
};
pub(crate) use self::runtime_helpers::{
    close_order_side_from_text, required_action_candle_count_for_timeframe,
    strategy_config_json_for_evaluate,
};
pub use self::storage::{query_live_order_context, query_live_orders};

pub use self::types::{LiveExecutionLogEntry, LiveStrategyConfig, LiveStrategyStatus};

pub async fn query_live_execution_logs(
    pool: &sqlx::SqlitePool,
    limit: i64,
    mode: &str,
    run_id: &str,
) -> crate::error::AppResult<Vec<LiveExecutionLogEntry>> {
    self::storage::query_live_execution_logs(pool, limit, mode, run_id).await
}

pub async fn query_live_execution_plans(
    pool: &sqlx::SqlitePool,
    limit: i64,
    mode: &str,
    run_id: &str,
) -> crate::error::AppResult<Vec<serde_json::Value>> {
    self::storage::query_live_execution_plans(pool, limit, mode, run_id).await
}

impl LiveStrategyRuntime {
    pub(crate) fn notify_planned_exit_worker(&self) {
        self.inner.planned_exit_notify.notify_one();
    }
}

#[derive(Clone)]
pub struct LiveStrategyRuntime {
    inner: Arc<LiveStrategyInner>,
}

struct LiveStrategyInner {
    status: RwLock<LiveStrategyStatus>,
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
    submitted_action_keys: Mutex<HashSet<String>>,
    synced_leverage_keys: Mutex<HashSet<String>>,
    planned_exit_notify: Notify,
    execution_logs: Mutex<VecDeque<LiveExecutionLogEntry>>,
    execution_log_seq: Mutex<u64>,
    execution_log_db: RwLock<Option<sqlx::SqlitePool>>,
}
