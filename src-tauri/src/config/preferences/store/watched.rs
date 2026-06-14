use crate::error::{AppError, AppResult};

use super::super::{
    watched::{
        apply_sync_days_to_plans, has_enabled_sync_plan, infer_sync_days_from_plans,
        normalize_sync_days, normalize_sync_plans_from_slice, normalize_watched_symbol,
        normalize_watched_symbols, AddWatchedSymbolRequest, WatchedSymbolRecord,
        WatchedSymbolUpsertResult,
    },
    WATCHED_SYMBOLS_KEY,
};
use super::PreferencesStore;

impl PreferencesStore {
    pub async fn watched_symbols(&self) -> AppResult<Vec<WatchedSymbolRecord>> {
        let mut state = self.lock.lock().await;
        if let Some(records) = state.watched_symbols.as_ref() {
            return Ok(records.clone());
        }
        let data = self.load_cached_unlocked(&mut state)?;
        let records = normalize_watched_symbols(data.get(WATCHED_SYMBOLS_KEY));
        state.watched_symbols = Some(records.clone());
        Ok(records)
    }

    pub async fn add_watched_symbol(
        &self,
        req: AddWatchedSymbolRequest,
    ) -> AppResult<WatchedSymbolUpsertResult> {
        let mut state = self.lock.lock().await;
        let mut data = self.load_cached_unlocked(&mut state)?;
        let mut records = normalize_watched_symbols(data.get(WATCHED_SYMBOLS_KEY));
        let symbol = normalize_watched_symbol(&req.symbol)
            .ok_or_else(|| AppError::Validation("币种不能为空".to_string()))?;
        let mut existed = false;
        let initial_sync_spot = req.sync_spot.unwrap_or(true);
        let initial_sync_swap = req.sync_swap.unwrap_or(true);
        if !initial_sync_spot && !initial_sync_swap {
            return Err(AppError::Validation(
                "至少需要选择现货或永续中的一种同步目标".to_string(),
            ));
        }
        let initial_sync_plans = req
            .sync_plans
            .as_ref()
            .map(|plans| normalize_sync_plans_from_slice(plans))
            .unwrap_or_default();
        let initial_sync_days = req
            .sync_days
            .map(normalize_sync_days)
            .unwrap_or_else(|| infer_sync_days_from_plans(&initial_sync_plans));
        let initial_sync_plans = apply_sync_days_to_plans(initial_sync_plans, initial_sync_days);
        if req.sync_plans.is_some() && !has_enabled_sync_plan(&initial_sync_plans) {
            return Err(AppError::Validation(
                "至少需要启用一个 K 线采集周期".to_string(),
            ));
        }
        let mut next_record = WatchedSymbolRecord::new(
            &symbol,
            initial_sync_spot,
            initial_sync_swap,
            req.archive_all_history.unwrap_or(false),
            initial_sync_days,
            initial_sync_plans,
            None,
        );

        if let Some(existing) = records.iter_mut().find(|item| item.symbol == symbol) {
            existed = true;
            let next_sync_spot = req.sync_spot.unwrap_or(existing.sync_spot);
            let next_sync_swap = req.sync_swap.unwrap_or(existing.sync_swap);
            if !next_sync_spot && !next_sync_swap {
                return Err(AppError::Validation(
                    "至少需要选择现货或永续中的一种同步目标".to_string(),
                ));
            }
            let next_archive_all_history = req
                .archive_all_history
                .unwrap_or(existing.archive_all_history);
            let next_sync_plans = req
                .sync_plans
                .as_ref()
                .map(|plans| normalize_sync_plans_from_slice(plans))
                .unwrap_or_else(|| existing.sync_plans.clone());
            let next_sync_days = req.sync_days.map(normalize_sync_days).unwrap_or_else(|| {
                if req.sync_plans.is_some() {
                    infer_sync_days_from_plans(&next_sync_plans)
                } else {
                    existing.sync_days
                }
            });
            let next_sync_plans = apply_sync_days_to_plans(next_sync_plans, next_sync_days);
            if req.sync_plans.is_some() && !has_enabled_sync_plan(&next_sync_plans) {
                return Err(AppError::Validation(
                    "至少需要启用一个 K 线采集周期".to_string(),
                ));
            }
            let config_changed = existing.sync_spot != next_sync_spot
                || existing.sync_swap != next_sync_swap
                || existing.archive_all_history != next_archive_all_history
                || existing.sync_days != next_sync_days
                || existing.sync_plans != next_sync_plans;
            next_record = WatchedSymbolRecord::new(
                &symbol,
                next_sync_spot,
                next_sync_swap,
                next_archive_all_history,
                next_sync_days,
                next_sync_plans,
                Some(existing),
            );
            if config_changed {
                next_record.updated_at = chrono::Utc::now().to_rfc3339();
            } else {
                next_record.updated_at = existing.updated_at.clone();
            }
            *existing = next_record.clone();
        } else {
            records.push(next_record.clone());
        }

        data.insert(
            WATCHED_SYMBOLS_KEY.to_string(),
            serde_json::to_value(&records)?,
        );
        self.save_cached_unlocked(&mut state, &data)?;
        state.watched_symbols = Some(records.clone());
        tracing::info!(
            symbol,
            existed,
            sync_spot = next_record.sync_spot,
            sync_swap = next_record.sync_swap,
            archive_all_history = next_record.archive_all_history,
            sync_days = next_record.sync_days,
            sync_plan_count = next_record.sync_plans.len(),
            total = records.len(),
            "watched symbol saved"
        );
        Ok(WatchedSymbolUpsertResult {
            watched_symbol: next_record,
            existed,
        })
    }

    pub async fn remove_watched_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<WatchedSymbolRecord>> {
        let mut state = self.lock.lock().await;
        let mut data = self.load_cached_unlocked(&mut state)?;
        let Some(normalized) = normalize_watched_symbol(symbol) else {
            return Ok(None);
        };
        let mut records = normalize_watched_symbols(data.get(WATCHED_SYMBOLS_KEY));
        let before_len = records.len();
        let mut removed = None;
        records.retain(|item| {
            if item.symbol == normalized {
                removed = Some(item.clone());
                false
            } else {
                true
            }
        });
        if records.len() != before_len {
            data.insert(
                WATCHED_SYMBOLS_KEY.to_string(),
                serde_json::to_value(&records)?,
            );
            self.save_cached_unlocked(&mut state, &data)?;
            state.watched_symbols = Some(records.clone());
            tracing::info!(
                symbol = normalized,
                total = records.len(),
                "watched symbol removed"
            );
        } else {
            state.watched_symbols = Some(records.clone());
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::*;

    #[tokio::test]
    async fn watched_symbols_cache_updates_after_mutation() {
        let path = temp_preferences_path("watched_cache_updates");
        write_watched_preferences(&path, 1);
        let store = PreferencesStore::new(path.clone());

        let initial = store.watched_symbols().await.expect("initial watched");
        assert_eq!(initial.len(), 1);

        store
            .add_watched_symbol(AddWatchedSymbolRequest {
                symbol: "ETH-USDT".to_string(),
                sync_spot: Some(true),
                sync_swap: Some(false),
                archive_all_history: Some(false),
                sync_days: Some(30),
                sync_plans: None,
            })
            .await
            .expect("add watched symbol");
        let added = store.watched_symbols().await.expect("watched after add");
        assert_eq!(added.len(), 2);
        assert!(added.iter().any(|record| record.symbol == "ETH-USDT"));

        let removed = store
            .remove_watched_symbol("ETH-USDT")
            .await
            .expect("remove watched symbol");
        assert!(removed.is_some());
        let after_remove = store.watched_symbols().await.expect("watched after remove");
        assert_eq!(after_remove.len(), 1);
        assert!(!after_remove
            .iter()
            .any(|record| record.symbol == "ETH-USDT"));

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn watched_symbols_returns_cached_snapshot_after_initial_load() {
        let path = temp_preferences_path("watched_cached_snapshot");
        write_watched_preferences(&path, 2);
        let store = PreferencesStore::new(path.clone());

        let initial = store.watched_symbols().await.expect("initial watched");
        assert_eq!(initial.len(), 2);

        write_watched_preferences(&path, 5);
        let cached = store.watched_symbols().await.expect("cached watched");

        assert_eq!(watched_symbols(&cached), watched_symbols(&initial));
        let _ = std::fs::remove_file(path);
    }

    fn temp_preferences_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "okxq_preferences_{label}_{}.json",
            uuid::Uuid::new_v4().simple()
        ))
    }

    fn write_watched_preferences(path: &std::path::Path, symbol_count: usize) {
        let mut data = Map::new();
        let watched = (0..symbol_count)
            .map(watched_symbol_value)
            .collect::<Vec<_>>();
        data.insert(WATCHED_SYMBOLS_KEY.to_string(), Value::Array(watched));
        let content = serde_json::to_string(&Value::Object(data)).expect("preferences json");
        std::fs::write(path, content).expect("write watched preferences");
    }

    fn watched_symbol_value(index: usize) -> Value {
        let symbol = format!("PERF{index:04}-USDT");
        json!({
            "symbol": symbol,
            "sync_spot": index % 3 != 0,
            "sync_swap": true,
            "archive_all_history": index % 11 == 0,
            "sync_days": 90,
            "sync_plans": [
                { "timeframe": "1m", "enabled": true, "bootstrap_days": 90, "archive_mode": "rolling" },
                { "timeframe": "5m", "enabled": true, "bootstrap_days": 90, "archive_mode": "rolling" },
                { "timeframe": "1H", "enabled": true, "bootstrap_days": 180, "archive_mode": "rolling" },
                { "timeframe": "1D", "enabled": index % 5 == 0, "bootstrap_days": 365, "archive_mode": "full" }
            ],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        })
    }

    fn watched_symbols(records: &[WatchedSymbolRecord]) -> Vec<String> {
        records.iter().map(|record| record.symbol.clone()).collect()
    }
}
