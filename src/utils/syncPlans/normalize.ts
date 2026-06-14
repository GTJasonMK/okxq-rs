import type { Timeframe, WatchedSymbolSyncPlan } from '@/types'
import { DEFAULT_SYNC_DAYS, SYNC_TIMEFRAMES } from './constants'
import { defaultSyncPlan } from './defaults'
import { isSupportedTimeframe, timeframeOrder } from './timeframes'

export function normalizeSyncPlan(plan: WatchedSymbolSyncPlan): WatchedSymbolSyncPlan {
  const timeframe = isSupportedTimeframe(plan.timeframe) ? plan.timeframe : '1H'
  const days = typeof plan.bootstrap_days === 'number' && Number.isFinite(plan.bootstrap_days)
    ? Math.round(plan.bootstrap_days)
    : Number.NaN
  return {
    timeframe,
    enabled: plan.enabled === true,
    bootstrap_days: Number.isFinite(days) ? Math.max(1, Math.min(3650, days)) : DEFAULT_SYNC_DAYS[timeframe],
    archive_mode: plan.archive_mode === 'full' ? 'full' : 'rolling',
  }
}

export function normalizeFullSyncPlans(
  plans: WatchedSymbolSyncPlan[],
  missingPlan: (timeframe: Timeframe) => WatchedSymbolSyncPlan = defaultSyncPlan,
): WatchedSymbolSyncPlan[] {
  const byTimeframe = new Map<Timeframe, WatchedSymbolSyncPlan>()
  for (const plan of plans || []) {
    const normalized = normalizeSyncPlan(plan)
    byTimeframe.set(normalized.timeframe, normalized)
  }
  return SYNC_TIMEFRAMES.map(timeframe => byTimeframe.get(timeframe) ?? missingPlan(timeframe))
}

export function normalizeProvidedSyncPlans(plans: WatchedSymbolSyncPlan[]): WatchedSymbolSyncPlan[] {
  const byTimeframe = new Map<Timeframe, WatchedSymbolSyncPlan>()
  for (const plan of plans || []) {
    const normalized = normalizeSyncPlan(plan)
    byTimeframe.set(normalized.timeframe, normalized)
  }
  return Array.from(byTimeframe.values())
    .sort((a, b) => timeframeOrder(a.timeframe) - timeframeOrder(b.timeframe))
}

export function normalizeEnabledSyncPlans(plans: WatchedSymbolSyncPlan[]): WatchedSymbolSyncPlan[] {
  return normalizeFullSyncPlans(plans)
    .filter(plan => plan.enabled)
    .sort((a, b) => timeframeOrder(a.timeframe) - timeframeOrder(b.timeframe))
}

export function normalizeEnabledProvidedSyncPlans(plans: WatchedSymbolSyncPlan[]): WatchedSymbolSyncPlan[] {
  return normalizeProvidedSyncPlans(plans).filter(plan => plan.enabled)
}
