use serde::{Deserialize, Serialize};

use super::plans::normalize_sync_days;

pub(super) const DEFAULT_SYNC_DAYS: i64 = 90;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchedSymbolRecord {
    pub symbol: String,
    pub base_ccy: String,
    pub spot_inst_id: String,
    pub swap_inst_id: String,
    pub sync_spot: bool,
    pub sync_swap: bool,
    pub archive_all_history: bool,
    #[serde(default = "default_sync_days")]
    pub sync_days: i64,
    #[serde(default)]
    pub sync_plans: Vec<WatchedSymbolSyncPlan>,
    pub created_at: String,
    pub updated_at: String,
}

impl WatchedSymbolRecord {
    pub(in crate::config::preferences) fn new(
        symbol: &str,
        sync_spot: bool,
        sync_swap: bool,
        archive_all_history: bool,
        sync_days: i64,
        sync_plans: Vec<WatchedSymbolSyncPlan>,
        existing: Option<&WatchedSymbolRecord>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let base_ccy = symbol.split('-').next().unwrap_or(symbol).to_string();
        Self {
            symbol: symbol.to_string(),
            base_ccy,
            spot_inst_id: symbol.to_string(),
            swap_inst_id: format!("{symbol}-SWAP"),
            sync_spot,
            sync_swap,
            archive_all_history,
            sync_days: normalize_sync_days(sync_days),
            sync_plans,
            created_at: existing
                .map(|item| item.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchedSymbolSyncPlan {
    pub timeframe: String,
    pub enabled: bool,
    pub bootstrap_days: i64,
    pub archive_mode: String,
}

#[derive(Debug, Deserialize)]
pub struct AddWatchedSymbolRequest {
    pub symbol: String,
    pub sync_spot: Option<bool>,
    pub sync_swap: Option<bool>,
    pub archive_all_history: Option<bool>,
    pub sync_days: Option<i64>,
    pub sync_plans: Option<Vec<WatchedSymbolSyncPlan>>,
}

#[derive(Debug, Serialize)]
pub struct WatchedSymbolUpsertResult {
    pub watched_symbol: WatchedSymbolRecord,
    pub existed: bool,
}

fn default_sync_days() -> i64 {
    DEFAULT_SYNC_DAYS
}
