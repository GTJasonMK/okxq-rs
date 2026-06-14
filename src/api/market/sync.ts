import { apiGet, apiPost, apiPut } from '../client'
import { arrayRecords } from '../normalize'
import type {
  SyncJob,
  MarketGapPlan,
  MarketGapPlanRequest,
  MarketGapRepairRequest,
  SyncRecord,
  SyncRuntimeSettings,
} from '@/types/market'
import {
  normalizeMarketGapPlan,
  normalizeSyncRecord,
  normalizeSyncJob,
  normalizeSyncRuntimeConfig,
} from '../marketNormalize'

export function fetchMarketGapPlan(params: MarketGapPlanRequest): Promise<MarketGapPlan> {
  return apiPost<unknown>('/api/market/gaps/plan', params).then(normalizeMarketGapPlan)
}

export function startGapRepairJob(params: MarketGapRepairRequest): Promise<SyncJob> {
  return apiPost<unknown>('/api/market/gaps/repair/jobs', params).then(normalizeSyncJob)
}

export function fetchSyncJobs(
  params?: { active_only?: boolean; limit?: number; task_ids?: string[]; watched_only?: boolean },
  options?: { dedupe?: boolean },
) {
  return apiGet<unknown>('/api/market/sync/jobs', params, options)
    .then(data => arrayRecords(data).map(normalizeSyncJob))
}

export function fetchSyncRecords(params?: { watched_only?: boolean }): Promise<SyncRecord[]> {
  return apiGet<unknown>('/api/market/sync/records', params)
    .then(data => arrayRecords(data)
      .map(normalizeSyncRecord)
      .filter((record): record is SyncRecord => Boolean(record)))
}

export function cancelSyncJob(taskId: string) {
  return apiPost(`/api/market/sync/jobs/${taskId}/cancel`)
}

export function fetchSyncRuntimeConfig() {
  return apiGet<unknown>('/api/market/sync/config').then(normalizeSyncRuntimeConfig)
}

export function updateSyncRuntimeConfig(settings: SyncRuntimeSettings) {
  return apiPut<unknown>('/api/market/sync/config', { settings }).then(normalizeSyncRuntimeConfig)
}
