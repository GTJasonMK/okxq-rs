import type {
  SyncJob,
  SyncRecord,
  SyncRuntimeConfig,
  SyncRuntimeSettings,
} from '@/types/market'
import {
  booleanValue,
  nullableTimestampNumber as optionalTimestampValue,
  numberValue,
  recordFrom,
  stringValue as textValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeBaseSymbol,
  normalizeTimeframe,
  normalizeTimeframeList,
} from './core'

export function normalizeSyncRuntimeConfig(raw: unknown): SyncRuntimeConfig {
  const item = recordFrom(raw)
  const settings = normalizeSyncRuntimeSettings(item.settings)
  return {
    settings,
    defaults: normalizeSyncRuntimeSettings(item.defaults, settings),
    limits: normalizeSyncRuntimeLimits(item.limits),
    active_sync_jobs: numberValue(item.active_sync_jobs),
    message: textValue(item.message),
  }
}

function normalizeSyncRuntimeSettings(raw: unknown, baseSettings?: SyncRuntimeSettings): SyncRuntimeSettings {
  const item = recordFrom(raw)
  const base = baseSettings ?? {
    max_sync_batches: 2000,
    okx_page_pause_ms: 0,
    sync_job_concurrency: 2,
    window_fetch_concurrency: 8,
    window_fetch_batches_per_slice: 32,
    candle_upsert_transaction_chunk: 1000,
    okx_max_concurrency: 10,
    okx_public_rest_concurrency: 8,
    okx_private_rest_concurrency: 2,
    okx_trade_rest_concurrency: 2,
    okx_ws_control_concurrency: 1,
    okx_unknown_concurrency: 1,
  }
  return {
    max_sync_batches: clampInt(item.max_sync_batches, base.max_sync_batches, 1, 20_000),
    okx_page_pause_ms: clampInt(item.okx_page_pause_ms, base.okx_page_pause_ms, 0, 5_000),
    sync_job_concurrency: clampInt(item.sync_job_concurrency, base.sync_job_concurrency, 1, 16),
    window_fetch_concurrency: clampInt(item.window_fetch_concurrency, base.window_fetch_concurrency, 1, 32),
    window_fetch_batches_per_slice: clampInt(item.window_fetch_batches_per_slice, base.window_fetch_batches_per_slice, 1, 256),
    candle_upsert_transaction_chunk: clampInt(item.candle_upsert_transaction_chunk, base.candle_upsert_transaction_chunk, 100, 10_000),
    okx_max_concurrency: clampInt(item.okx_max_concurrency, base.okx_max_concurrency, 1, 64),
    okx_public_rest_concurrency: clampInt(item.okx_public_rest_concurrency, base.okx_public_rest_concurrency, 1, 64),
    okx_private_rest_concurrency: clampInt(item.okx_private_rest_concurrency, base.okx_private_rest_concurrency, 1, 32),
    okx_trade_rest_concurrency: clampInt(item.okx_trade_rest_concurrency, base.okx_trade_rest_concurrency, 1, 16),
    okx_ws_control_concurrency: clampInt(item.okx_ws_control_concurrency, base.okx_ws_control_concurrency, 1, 8),
    okx_unknown_concurrency: clampInt(item.okx_unknown_concurrency, base.okx_unknown_concurrency, 1, 16),
  }
}

function normalizeSyncRuntimeLimits(raw: unknown): SyncRuntimeConfig['limits'] {
  const item = recordFrom(raw)
  const limits: SyncRuntimeConfig['limits'] = {}
  for (const key of [
    'max_sync_batches',
    'okx_page_pause_ms',
    'sync_job_concurrency',
    'window_fetch_concurrency',
    'window_fetch_batches_per_slice',
    'candle_upsert_transaction_chunk',
    'okx_max_concurrency',
    'okx_public_rest_concurrency',
    'okx_private_rest_concurrency',
    'okx_trade_rest_concurrency',
    'okx_ws_control_concurrency',
    'okx_unknown_concurrency',
  ] as Array<keyof SyncRuntimeSettings>) {
    const entry = recordFrom(item[key])
    if (Object.keys(entry).length === 0) continue
    limits[key] = {
      min: numberValue(entry.min),
      max: numberValue(entry.max),
    }
  }
  return limits
}

export function normalizeSyncJob(raw: unknown): SyncJob {
  const item = recordFrom(raw)
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeSyncInstType(item.inst_type, rawInstId)
  const timeframe = normalizeTimeframe(textValue(item.timeframe, '1m')) || '1m'
  const sourceTimeframe = normalizeTimeframe(textValue(item.source_timeframe, '1m')) || '1m'
  return {
    task_id: textValue(item.task_id),
    inst_id: normalizeSyncInstId(rawInstId, instType),
    inst_type: instType,
    timeframe: timeframe as SyncJob['timeframe'],
    source_timeframe: sourceTimeframe as SyncJob['source_timeframe'],
    target_timeframes: normalizeTimeframeList(item.target_timeframes) as SyncJob['target_timeframes'],
    mode: textValue(item.mode, 'window'),
    status: textValue(item.status, 'queued'),
    progress: numberValue(item.progress),
    message: textValue(item.message),
    days: numberValue(item.days),
    start_ts: optionalTimestampValue(item.start_ts),
    end_ts: optionalTimestampValue(item.end_ts),
    repair_method: textValue(item.repair_method),
    reused_existing: booleanValue(item.reused_existing),
    saved_count: numberValue(item.saved_count),
    fetched_count: numberValue(item.fetched_count),
    target_fetch_count: numberValue(item.target_fetch_count),
    target_save_count: numberValue(item.target_save_count),
    inserted_count: numberValue(item.inserted_count),
    derived_count: numberValue(item.derived_count),
    target_derive_count: numberValue(item.target_derive_count),
    batches: numberValue(item.batches),
    target_batches: numberValue(item.target_batches),
    api_calls: numberValue(item.api_calls),
    candle_count: numberValue(item.candle_count),
    history_complete: booleanValue(item.history_complete),
    created_at: textValue(item.created_at),
    updated_at: textValue(item.updated_at),
    finished_at: textValue(item.finished_at) || null,
    error: textValue(item.error),
  }
}

export function normalizeSyncRecord(raw: unknown): SyncRecord | null {
  const item = recordFrom(raw)
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeSyncInstType(item.inst_type, rawInstId)
  const timeframe = normalizeTimeframe(textValue(item.timeframe))
  if (!timeframe) return null
  return {
    inst_id: normalizeSyncInstId(rawInstId, instType),
    inst_type: instType,
    timeframe,
    last_sync_time: optionalTextValue(item.last_sync_time),
    oldest_timestamp: optionalTimestampValue(item.oldest_timestamp),
    newest_timestamp: optionalTimestampValue(item.newest_timestamp),
    oldest_time: optionalTextValue(item.oldest_time),
    newest_time: optionalTextValue(item.newest_time),
    candle_count: numberValue(item.candle_count),
    expected_candle_count: numberValue(item.expected_candle_count),
    gap_count: numberValue(item.gap_count),
    coverage_ratio: numberValue(item.coverage_ratio),
    history_complete: booleanValue(item.history_complete),
    last_sync_mode: textValue(item.last_sync_mode),
  }
}

function normalizeSyncInstType(value: unknown, instId: string): SyncRecord['inst_type'] {
  const normalized = textValue(value, inferInstTypeFromId(instId)).trim().toUpperCase()
  if (normalized === 'SWAP' || normalized === 'FUTURES') return normalized
  return 'SPOT'
}

function normalizeSyncInstId(value: string, instType: SyncRecord['inst_type']): string {
  let normalized = instType === 'FUTURES'
    ? value.trim().toUpperCase()
    : normalizeBaseSymbol(value)
  if (!normalized) return ''
  if (instType === 'SWAP' && !normalized.endsWith('-SWAP')) {
    normalized = normalized.endsWith('-USDT') ? `${normalized}-SWAP` : `${normalized}-USDT-SWAP`
  }
  if (instType === 'SPOT' && normalized.endsWith('-SWAP')) {
    normalized = normalized.slice(0, -5)
  }
  return normalized
}

function optionalTextValue(value: unknown): string | null {
  const text = textValue(value).trim()
  return text || null
}

function clampInt(value: unknown, defaultValue: number, min: number, max: number): number {
  const parsed = Math.round(numberValue(value, defaultValue))
  return Math.max(min, Math.min(max, Number.isFinite(parsed) ? parsed : defaultValue))
}
