import type {
  MarketSymbol,
  PriceAlert,
  SyncJob,
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types/market'
import {
  booleanValue,
  isRecord,
  numberValue,
  recordFrom,
  stringValue as textValue,
  timestampNumber as timestampValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeBaseSymbol,
  normalizeInstId,
  normalizeInstType,
  normalizeTimeframe,
  normalizeTimeframeList,
  timeframeOrder,
} from './core'
import { normalizeSyncJob } from './sync'

export function normalizeMarketSymbol(raw: unknown): MarketSymbol {
  const item = recordFrom(raw)
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  const instId = normalizeInstId(rawInstId, instType)
  const symbol = normalizeBaseSymbol(textValue(item.symbol) || textValue(item.base_ccy) || instId)
  return {
    symbol,
    base_ccy: textValue(item.base_ccy, symbol.split('-')[0] ?? ''),
    inst_id: instId,
    inst_type: instType,
    timeframes: normalizeTimeframeList(item.timeframes) as MarketSymbol['timeframes'],
    candle_count: numberValue(item.candle_count),
    managed: booleanValue(item.managed),
    watched: booleanValue(item.watched),
  }
}

export function normalizeWatchedSymbol(raw: Record<string, unknown>): WatchedSymbol {
  const symbol = normalizeBaseSymbol(textValue(raw.symbol))
  const syncPlans = normalizeWatchedSyncPlans(raw.sync_plans)
  return {
    symbol,
    base_ccy: textValue(raw.base_ccy, symbol.split('-')[0] ?? ''),
    spot_inst_id: normalizeInstId(textValue(raw.spot_inst_id, symbol), 'SPOT'),
    swap_inst_id: normalizeInstId(textValue(raw.swap_inst_id, `${symbol}-SWAP`), 'SWAP'),
    sync_spot: booleanValue(raw.sync_spot, true),
    sync_swap: booleanValue(raw.sync_swap, true),
    archive_all_history: booleanValue(raw.archive_all_history),
    sync_days: normalizeWatchedSyncDays(raw.sync_days, inferWatchedSyncDays(syncPlans)),
    sync_plans: syncPlans,
    created_at: textValue(raw.created_at),
    updated_at: textValue(raw.updated_at),
  }
}

export function normalizePriceAlert(raw: unknown): PriceAlert {
  const item = recordFrom(raw)
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  const alertType = normalizeAlertType(item.alert_type)
  const direction = normalizeAlertDirection(item.direction)
  const targetPrice = optionalNumber(item.target_price)
  const changePercent = optionalNumber(item.change_percent)
  const triggeredAt = textValue(item.triggered_at) || null
  return {
    id: textValue(item.id),
    inst_id: normalizeInstId(rawInstId, instType),
    symbol: textValue(item.symbol),
    inst_type: instType,
    alert_type: alertType,
    direction,
    target_price: targetPrice,
    change_percent: changePercent,
    note: textValue(item.note),
    enabled: booleanValue(item.enabled, true),
    trigger_once: booleanValue(item.trigger_once, true),
    cooldown_seconds: Math.max(0, Math.round(numberValue(item.cooldown_seconds, 300))),
    created_at: textValue(item.created_at),
    updated_at: textValue(item.updated_at),
    triggered_at: triggeredAt,
    last_value: optionalNumber(item.last_value),
    last_trigger_value: optionalNumber(item.last_trigger_value),
    last_trigger_ts: timestampValue(item.last_trigger_ts),
  }
}

type WatchMutationNormalizedKey =
  | 'watched_symbol'
  | 'sync_jobs'
  | 'cancelled_disabled_jobs'
  | 'existed'
  | 'started_count'
  | 'reused_count'
  | 'exact_gap_jobs'
  | 'rule_jobs'

type NormalizedWatchMutationResult<T> = Omit<T, WatchMutationNormalizedKey> & {
  watched_symbol?: WatchedSymbol
  sync_jobs: SyncJob[]
  cancelled_disabled_jobs: SyncJob[]
  existed?: boolean
  started_count?: number
  reused_count?: number
  exact_gap_jobs?: number
  rule_jobs?: number
}

export function normalizeWatchMutationResult<T extends Record<string, unknown>>(
  result: T,
): NormalizedWatchMutationResult<T> {
  const item = recordFrom(result)
  const watchedSymbol = item.watched_symbol
  const syncJobs = item.sync_jobs
  const cancelledDisabledJobs = item.cancelled_disabled_jobs
  return {
    ...result,
    watched_symbol: watchedSymbol !== undefined
      ? normalizeWatchedSymbol(recordFrom(watchedSymbol))
      : undefined,
    sync_jobs: Array.isArray(syncJobs) ? syncJobs.map(normalizeSyncJob) : [],
    cancelled_disabled_jobs: Array.isArray(cancelledDisabledJobs)
      ? cancelledDisabledJobs.map(normalizeSyncJob)
      : [],
    ...(item.existed !== undefined ? { existed: booleanValue(item.existed) } : {}),
    ...(item.started_count !== undefined
      ? { started_count: numberValue(item.started_count) }
      : {}),
    ...(item.reused_count !== undefined
      ? { reused_count: numberValue(item.reused_count) }
      : {}),
    ...(item.exact_gap_jobs !== undefined
      ? { exact_gap_jobs: numberValue(item.exact_gap_jobs) }
      : {}),
    ...(item.rule_jobs !== undefined
      ? { rule_jobs: numberValue(item.rule_jobs) }
      : {}),
  }
}

function normalizeWatchedSyncPlans(value: unknown): WatchedSymbolSyncPlan[] {
  if (!Array.isArray(value)) return []
  const seen = new Set<string>()
  return value
    .map((raw) => {
      const item = isRecord(raw) ? raw : {}
      const timeframe = normalizeTimeframe(textValue(item.timeframe))
      if (!timeframe || seen.has(timeframe)) return null
      seen.add(timeframe)
      const archiveMode = textValue(item.archive_mode).trim().toLowerCase() === 'full'
        ? 'full'
        : 'rolling'
      return {
        timeframe,
        enabled: booleanValue(item.enabled, true),
        bootstrap_days: Math.max(1, Math.min(3650, Math.round(numberValue(item.bootstrap_days, 90)))),
        archive_mode: archiveMode,
      } satisfies WatchedSymbolSyncPlan
    })
    .filter((plan): plan is WatchedSymbolSyncPlan => !!plan)
    .sort((a, b) => timeframeOrder(a.timeframe) - timeframeOrder(b.timeframe))
}

function normalizeWatchedSyncDays(value: unknown, defaultDays = 90): number {
  const parsed = Math.round(numberValue(value, Number.NaN))
  const baseDays = Math.round(numberValue(defaultDays, 30))
  if (Number.isFinite(parsed)) return Math.max(1, Math.min(3650, parsed))
  return Number.isFinite(baseDays) ? Math.max(1, Math.min(3650, baseDays)) : 90
}

function inferWatchedSyncDays(plans: WatchedSymbolSyncPlan[]): number {
  const enabled = plans.filter(plan => plan.enabled)
  if (enabled.length === 0) return 90
  return normalizeWatchedSyncDays(Math.max(...enabled.map(plan => plan.bootstrap_days)))
}

function normalizeAlertType(value: unknown): PriceAlert['alert_type'] {
  return textValue(value).trim().toLowerCase() === 'change' ? 'change' : 'price'
}

function normalizeAlertDirection(value: unknown): PriceAlert['direction'] {
  return textValue(value).trim().toLowerCase() === 'below' ? 'below' : 'above'
}

function optionalNumber(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const parsed = numberValue(value, Number.NaN)
  return Number.isFinite(parsed) ? parsed : null
}
