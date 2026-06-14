import type {
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import {
  ensureDerivedBaseSyncPlans,
  inferUnifiedSyncDays,
  normalizeEnabledProvidedSyncPlans,
} from '@/utils/syncPlans'
import { isInventoryOnlyRow } from '@/utils/dataCenter/watchlist/guards'

export function effectivePlansForRow(row: WatchedSymbol, enabledPlans: WatchedSymbolSyncPlan[]): WatchedSymbolSyncPlan[] {
  if (isInventoryOnlyRow(row)) return []
  const customPlans = normalizeEnabledProvidedSyncPlans(row.sync_plans ?? [])
  const plans = row.sync_plans && row.sync_plans.length > 0 ? customPlans : enabledPlans
  return ensureDerivedBaseSyncPlans(plans, row.sync_days ?? inferUnifiedSyncDays(plans))
}
