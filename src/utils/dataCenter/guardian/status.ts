import {
  normalizeSyncJob,
} from '@/api/marketNormalize'
import type {
  GuardianStatus,
} from '@/types/dataCenter'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  normalizeStringList,
  nullableTimestampValue,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'

export function normalizeGuardianStatus(value: unknown): GuardianStatus {
  const raw = recordFrom(value)
  return {
    enabled: booleanValue(raw.enabled),
    active: booleanValue(raw.active),
    policy_summary: stringValue(raw.policy_summary),
    rolling_window_timeframes: normalizeStringList(raw.rolling_window_timeframes),
    full_backfill_timeframes: normalizeStringList(raw.full_backfill_timeframes),
    watched_count: numberValue(raw.watched_count),
    backfill_queue_size: numberValue(raw.backfill_queue_size),
    current_inst_id: stringValue(raw.current_inst_id),
    current_timeframe: stringValue(raw.current_timeframe),
    current_mode: stringValue(raw.current_mode),
    current_phase: stringValue(raw.current_phase),
    last_successful_run_at: nullableTimestampValue(raw.last_successful_run_at),
    last_run_finished_at: nullableTimestampValue(raw.last_run_finished_at),
    backfill_queue_preview: arrayRecords(raw.backfill_queue_preview).map(normalizeSyncJob),
    last_errors: arrayValue(raw.last_errors),
    last_sync_results: arrayRecords(raw.last_sync_results).map(normalizeSyncJob),
  }
}
