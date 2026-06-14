import type {
  Timeframe,
  WatchedSymbolSyncPlan,
} from '@/types'
import type {
  GuardianPlan,
} from '@/types/dataCenter'
import {
  DEFAULT_UNIFIED_SYNC_DAYS,
  normalizeEnabledProvidedSyncPlans,
  normalizeEnabledSyncPlans,
  normalizeSyncDays,
  timeframeOrder,
} from '@/utils/syncPlans'

export function enabledSyncPlansFromGuardian(guardianPlans: GuardianPlan[]) {
  const plans = guardianPlans
    .map(guardianPlanToSyncPlan)
    .filter((plan): plan is WatchedSymbolSyncPlan => !!plan)
  const enabled = normalizeEnabledProvidedSyncPlans(plans)
  return enabled.length > 0 ? enabled : normalizeEnabledSyncPlans([])
}

export function guardianPlanToSyncPlan(plan: GuardianPlan): WatchedSymbolSyncPlan | null {
  if (timeframeOrder(plan.timeframe) >= 999) return null
  return {
    timeframe: plan.timeframe as Timeframe,
    enabled: plan.enabled === true,
    bootstrap_days: normalizeGuardianSyncDays(plan.bootstrap_days),
    archive_mode: plan.archive_mode === 'full' ? 'full' : 'rolling',
  }
}

function normalizeGuardianSyncDays(value: unknown): number {
  if (value === null || value === undefined) return DEFAULT_UNIFIED_SYNC_DAYS
  if (typeof value === 'string' && value.trim() === '') return DEFAULT_UNIFIED_SYNC_DAYS
  return normalizeSyncDays(value)
}
