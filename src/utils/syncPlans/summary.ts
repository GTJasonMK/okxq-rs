import type { WatchedSymbolSyncPlan } from '@/types'
import { inferUnifiedSyncDays } from './days'
import { normalizeEnabledProvidedSyncPlans } from './normalize'

export function syncPlanSummary(plans: WatchedSymbolSyncPlan[]): string {
  const enabled = normalizeEnabledProvidedSyncPlans(plans)
  if (enabled.length === 0) return '无周期'
  const days = inferUnifiedSyncDays(enabled)
  return `${enabled.map(plan => plan.timeframe).join('/')} · ${days}天`
}

export function sameSyncPlans(left: WatchedSymbolSyncPlan[], right: WatchedSymbolSyncPlan[]): boolean {
  if (left.length !== right.length) return false
  return left.every((plan, index) => {
    const other = right[index]
    return other &&
      plan.timeframe === other.timeframe &&
      plan.enabled === other.enabled &&
      plan.bootstrap_days === other.bootstrap_days &&
      plan.archive_mode === other.archive_mode
  })
}
