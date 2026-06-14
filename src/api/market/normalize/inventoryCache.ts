import type {
  InventoryCacheRebuildProgress,
  InventoryCacheRebuildResult,
  InventoryCacheRebuildStartResult,
  InventoryCacheRebuildStatus,
} from '@/types/dataCenter'
import {
  isRecord,
  numberValue,
  stringValue,
} from '../../normalize'
import {
  normalizeInventoryPayload,
} from '@/utils/dataCenter'

export function normalizeInventoryCacheRebuildResult(raw: unknown): InventoryCacheRebuildResult {
  const item = isRecord(raw) ? raw : {}
  return {
    message: stringValue(item.message),
    candle_groups_scanned: numberValue(item.candle_groups_scanned),
    sync_records_rebuilt: numberValue(item.sync_records_rebuilt),
    stale_sync_records_deleted: numberValue(item.stale_sync_records_deleted),
    sync_records_total: numberValue(item.sync_records_total),
    cached_candles_total: numberValue(item.cached_candles_total),
    inventory: normalizeInventoryPayload(item.inventory),
    progress: normalizeOptionalInventoryCacheRebuildProgress(item.progress),
  }
}

export function normalizeInventoryCacheRebuildStartResult(raw: unknown): InventoryCacheRebuildStartResult {
  const item = isRecord(raw) ? raw : {}
  return {
    reused_existing: Boolean(item.reused_existing),
    progress: normalizeOptionalInventoryCacheRebuildProgress(item.progress),
  }
}

export function normalizeInventoryCacheRebuildStatus(raw: unknown): InventoryCacheRebuildStatus {
  const item = isRecord(raw) ? raw : {}
  return {
    progress: normalizeOptionalInventoryCacheRebuildProgress(item.progress),
  }
}

function normalizeOptionalInventoryCacheRebuildProgress(raw: unknown): InventoryCacheRebuildProgress | null {
  if (!isRecord(raw)) return null
  return {
    task_id: stringValue(raw.task_id),
    status: stringValue(raw.status),
    phase: stringValue(raw.phase),
    progress: numberValue(raw.progress),
    message: stringValue(raw.message),
    started_at: stringValue(raw.started_at),
    updated_at: stringValue(raw.updated_at),
    finished_at: typeof raw.finished_at === 'string' ? raw.finished_at : null,
    error: stringValue(raw.error),
    processed_candles: numberValue(raw.processed_candles),
    target_candles: numberValue(raw.target_candles),
    processed_groups: numberValue(raw.processed_groups),
    target_groups: numberValue(raw.target_groups),
    scan_concurrency: numberValue(raw.scan_concurrency),
    candle_groups_scanned: numberValue(raw.candle_groups_scanned),
    sync_records_rebuilt: numberValue(raw.sync_records_rebuilt),
    stale_sync_records_deleted: numberValue(raw.stale_sync_records_deleted),
    sync_records_total: numberValue(raw.sync_records_total),
    cached_candles_total: numberValue(raw.cached_candles_total),
  }
}
