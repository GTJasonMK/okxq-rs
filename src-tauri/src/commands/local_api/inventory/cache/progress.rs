use std::sync::OnceLock;

use serde_json::{json, Value};

use super::InventoryCacheRebuildReport;

static INVENTORY_REBUILD_PROGRESS: OnceLock<
    tokio::sync::RwLock<Option<InventoryCacheRebuildProgress>>,
> = OnceLock::new();

#[derive(Clone, Debug)]
pub(super) struct InventoryCacheRebuildProgress {
    pub(super) task_id: String,
    pub(super) status: String,
    pub(super) phase: String,
    pub(super) progress: i64,
    pub(super) message: String,
    pub(super) started_at: String,
    pub(super) updated_at: String,
    pub(super) finished_at: Option<String>,
    pub(super) error: String,
    pub(super) processed_candles: i64,
    pub(super) target_candles: i64,
    pub(super) processed_groups: i64,
    pub(super) target_groups: i64,
    pub(super) scan_concurrency: i64,
    pub(super) candle_groups_scanned: i64,
    pub(super) sync_records_rebuilt: i64,
    pub(super) stale_sync_records_deleted: i64,
    pub(super) sync_records_total: i64,
    pub(super) cached_candles_total: i64,
}

impl InventoryCacheRebuildProgress {
    pub(super) fn queued(task_id: String) -> Self {
        let now = now_text();
        Self {
            task_id,
            status: "queued".to_string(),
            phase: "queued".to_string(),
            progress: 0,
            message: "等待开始全库扫描".to_string(),
            started_at: now.clone(),
            updated_at: now,
            finished_at: None,
            error: String::new(),
            processed_candles: 0,
            target_candles: 0,
            processed_groups: 0,
            target_groups: 0,
            scan_concurrency: 0,
            candle_groups_scanned: 0,
            sync_records_rebuilt: 0,
            stale_sync_records_deleted: 0,
            sync_records_total: 0,
            cached_candles_total: 0,
        }
    }

    pub(super) fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "queued" | "running")
    }

    pub(super) fn to_value(&self) -> Value {
        json!({
            "task_id": self.task_id,
            "status": self.status,
            "phase": self.phase,
            "progress": self.progress,
            "message": self.message,
            "started_at": self.started_at,
            "updated_at": self.updated_at,
            "finished_at": self.finished_at,
            "error": self.error,
            "processed_candles": self.processed_candles,
            "target_candles": self.target_candles,
            "processed_groups": self.processed_groups,
            "target_groups": self.target_groups,
            "scan_concurrency": self.scan_concurrency,
            "candle_groups_scanned": self.candle_groups_scanned,
            "sync_records_rebuilt": self.sync_records_rebuilt,
            "stale_sync_records_deleted": self.stale_sync_records_deleted,
            "sync_records_total": self.sync_records_total,
            "cached_candles_total": self.cached_candles_total,
        })
    }
}

pub(super) fn next_rebuild_task_id() -> String {
    format!(
        "inventory_rebuild_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
}

pub(super) async fn current_rebuild_progress() -> Option<InventoryCacheRebuildProgress> {
    rebuild_progress_store().read().await.clone()
}

pub(super) async fn current_rebuild_progress_value() -> Value {
    current_rebuild_progress()
        .await
        .map(|progress| progress.to_value())
        .unwrap_or(Value::Null)
}

pub(super) async fn set_rebuild_progress(progress: InventoryCacheRebuildProgress) {
    *rebuild_progress_store().write().await = Some(progress);
}

pub(super) async fn update_rebuild_progress<F>(task_id: Option<&str>, mutate: F)
where
    F: FnOnce(&mut InventoryCacheRebuildProgress),
{
    let Some(task_id) = task_id else {
        return;
    };
    let mut guard = rebuild_progress_store().write().await;
    let Some(progress) = guard
        .as_mut()
        .filter(|progress| progress.task_id == task_id)
    else {
        return;
    };
    mutate(progress);
    progress.updated_at = now_text();
    tracing::debug!(
        task_id = %progress.task_id,
        status = %progress.status,
        phase = %progress.phase,
        progress = progress.progress,
        processed_candles = progress.processed_candles,
        target_candles = progress.target_candles,
        processed_groups = progress.processed_groups,
        target_groups = progress.target_groups,
        scan_concurrency = progress.scan_concurrency,
        sync_records_rebuilt = progress.sync_records_rebuilt,
        message = %progress.message,
        "inventory cache rebuild progress"
    );
}

pub(super) async fn mark_rebuild_completed(task_id: &str, report: &InventoryCacheRebuildReport) {
    update_rebuild_progress(Some(task_id), |progress| {
        progress.status = "completed".to_string();
        progress.phase = "completed".to_string();
        progress.progress = 100;
        progress.finished_at = Some(now_text());
        progress.error.clear();
        progress.candle_groups_scanned = report.candle_groups_scanned;
        progress.sync_records_rebuilt = report.sync_records_rebuilt;
        progress.stale_sync_records_deleted = report.stale_sync_records_deleted;
        progress.sync_records_total = report.sync_records_total;
        progress.cached_candles_total = report.cached_candles_total;
        progress.message = format!(
            "库存缓存重建完成：重建 {} 组，清理陈旧缓存 {} 条",
            report.sync_records_rebuilt, report.stale_sync_records_deleted
        );
    })
    .await;
}

pub(super) async fn mark_rebuild_failed(task_id: &str, error: String) {
    update_rebuild_progress(Some(task_id), |progress| {
        progress.status = "failed".to_string();
        progress.phase = "failed".to_string();
        progress.progress = 100;
        progress.finished_at = Some(now_text());
        progress.error = error.clone();
        progress.message = format!("库存缓存重建失败：{error}");
    })
    .await;
}

fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn rebuild_progress_store() -> &'static tokio::sync::RwLock<Option<InventoryCacheRebuildProgress>> {
    INVENTORY_REBUILD_PROGRESS.get_or_init(|| tokio::sync::RwLock::new(None))
}
