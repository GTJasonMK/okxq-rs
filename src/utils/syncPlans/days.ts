import type { Timeframe, WatchedSymbolSyncPlan } from '@/types'
import { BASE_SYNC_TIMEFRAME, DEFAULT_UNIFIED_SYNC_DAYS } from './constants'
import {
  normalizeEnabledProvidedSyncPlans,
  normalizeFullSyncPlans,
  normalizeProvidedSyncPlans,
} from './normalize'
import { timeframeOrder } from './timeframes'

export function normalizeSyncDays(value: unknown): number {
  const parsed = typeof value === 'number' && Number.isFinite(value)
    ? Math.round(value)
    : Number.NaN
  if (Number.isFinite(parsed)) return Math.max(1, Math.min(3650, parsed))
  return DEFAULT_UNIFIED_SYNC_DAYS
}

export function inferUnifiedSyncDays(plans: WatchedSymbolSyncPlan[]): number {
  const enabled = normalizeEnabledProvidedSyncPlans(plans)
  if (enabled.length === 0) return DEFAULT_UNIFIED_SYNC_DAYS
  const values = enabled.map(plan => normalizeSyncDays(plan.bootstrap_days))
  const first = values[0]
  if (values.every(value => value === first)) return first
  return normalizeSyncDays(Math.max(...values))
}

export function applyUnifiedSyncDays(plans: WatchedSymbolSyncPlan[], days: number): WatchedSymbolSyncPlan[] {
  const normalizedDays = normalizeSyncDays(days)
  return normalizeFullSyncPlans(plans).map(plan => ({
    ...plan,
    bootstrap_days: normalizedDays,
  }))
}

export function ensureDerivedBaseSyncPlans(
  plans: WatchedSymbolSyncPlan[],
  syncDays?: number,
): WatchedSymbolSyncPlan[] {
  const normalized = normalizeProvidedSyncPlans(plans)
  const byTimeframe = new Map<Timeframe, WatchedSymbolSyncPlan>()
  for (const plan of normalized) {
    byTimeframe.set(plan.timeframe, plan)
  }
  const currentBase = byTimeframe.get(BASE_SYNC_TIMEFRAME)
  const baseDays = normalizeSyncDays(syncDays ?? currentBase?.bootstrap_days ?? inferUnifiedSyncDays(normalized))
  byTimeframe.set(BASE_SYNC_TIMEFRAME, {
    timeframe: BASE_SYNC_TIMEFRAME,
    enabled: true,
    bootstrap_days: baseDays,
    archive_mode: currentBase?.archive_mode === 'full' ? 'full' : 'rolling',
  })
  return Array.from(byTimeframe.values())
    .sort((a, b) => timeframeOrder(a.timeframe) - timeframeOrder(b.timeframe))
}
