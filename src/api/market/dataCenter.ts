import { apiGet, apiPost } from '../client'
import type {
  InventoryCacheRebuildResult,
  InventoryCacheRebuildStartResult,
  InventoryCacheRebuildStatus,
} from '@/types/dataCenter'
import {
  normalizeGuardianConfig,
  normalizeGuardianStatus,
  normalizeInventoryPayload,
  normalizeTickCollectorStatus,
} from '@/utils/dataCenter'
import {
  normalizeInventoryCacheRebuildResult,
  normalizeInventoryCacheRebuildStartResult,
  normalizeInventoryCacheRebuildStatus,
  normalizeTickCollectorActionResult,
} from './normalize'

export function fetchInventory(opts?: { include_storage_counts?: boolean }) {
  return apiGet<unknown>('/api/market/inventory', opts).then(normalizeInventoryPayload)
}

export function rebuildInventoryCache(opts?: { include_storage_counts?: boolean; concurrency?: number }): Promise<InventoryCacheRebuildResult> {
  return apiPost<unknown>('/api/market/inventory/rebuild-cache', opts).then(normalizeInventoryCacheRebuildResult)
}

export function startInventoryCacheRebuild(opts?: { include_storage_counts?: boolean; background?: boolean; concurrency?: number }): Promise<InventoryCacheRebuildStartResult> {
  return apiPost<unknown>('/api/market/inventory/rebuild-cache', { background: true, ...opts })
    .then(normalizeInventoryCacheRebuildStartResult)
}

export function fetchInventoryCacheRebuildStatus(): Promise<InventoryCacheRebuildStatus> {
  return apiGet<unknown>('/api/market/inventory/rebuild-cache/status', undefined, { dedupe: false })
    .then(normalizeInventoryCacheRebuildStatus)
}

export function fetchGuardianStatus() {
  return apiGet<unknown>('/api/market/data-guardian/status').then(normalizeGuardianStatus)
}

export function fetchGuardianConfig() {
  return apiGet<unknown>('/api/market/data-guardian/config').then(normalizeGuardianConfig)
}

export function runDataGuardianNow() {
  return apiPost<unknown>('/api/market/data-guardian/run-now').then(normalizeGuardianStatus)
}

export function fetchTickCollectorStatus() {
  return apiGet<unknown>('/api/market/tick-collector/status').then(normalizeTickCollectorStatus)
}

export function startTickCollector() {
  return apiPost<unknown>('/api/market/tick-collector/start').then(normalizeTickCollectorActionResult)
}

export function stopTickCollector() {
  return apiPost<unknown>('/api/market/tick-collector/stop').then(normalizeTickCollectorActionResult)
}
