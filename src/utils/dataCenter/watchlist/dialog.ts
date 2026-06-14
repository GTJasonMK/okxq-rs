import type { WatchedSymbol, WatchedSymbolSyncPlan } from '@/types'
import type { GuardianPlan } from '@/types/dataCenter'
import {
  DEFAULT_UNIFIED_SYNC_DAYS,
  applyUnifiedSyncDays,
  disabledSyncPlan,
  inferUnifiedSyncDays,
  normalizeEnabledSyncPlans,
  normalizeFullSyncPlans,
  normalizeSyncDays,
} from '@/utils/syncPlans'
import { guardianPlanToSyncPlan } from '@/utils/dataCenter/guardian'
import { normalizeInputSymbol } from '@/utils/dataCenter/normalize'
import { isInventoryOnlyRow } from '@/utils/dataCenter/watchlist/guards'
import type { WatchRuleFormState } from '@/utils/dataCenter/watchlist/types'

export function canOpenWatchRuleDialog(symbol: string) {
  return !!normalizeInputSymbol(symbol)
}

export function canSubmitWatchRuleDialog(input: {
  pendingSymbol: string
  syncSpot: boolean
  syncSwap: boolean
  syncPlans: WatchedSymbolSyncPlan[]
}) {
  return (
    !!input.pendingSymbol &&
    (input.syncSpot || input.syncSwap) &&
    normalizeEnabledSyncPlans(input.syncPlans).length > 0
  )
}

export function watchRuleSubmitButtonLabel(adding: boolean, autoSync: boolean) {
  if (adding) return '保存中'
  return autoSync ? '保存规则并同步' : '保存关注规则'
}

export function defaultWatchRuleForm(
  guardianPlans: GuardianPlan[],
  syncDays = DEFAULT_UNIFIED_SYNC_DAYS,
): WatchRuleFormState {
  const normalizedSyncDays = normalizeSyncDays(syncDays)
  return {
    syncSpot: true,
    syncSwap: true,
    archiveAll: false,
    autoSync: true,
    syncDays: normalizedSyncDays,
    syncPlans: defaultEditablePlans(guardianPlans, normalizedSyncDays),
  }
}

export function watchRuleFormFromRow(
  row: WatchedSymbol,
  guardianPlans: GuardianPlan[],
  currentSyncDays: number,
): WatchRuleFormState {
  const plans = editablePlansForRow(row, guardianPlans, currentSyncDays)
  const syncDays = typeof row.sync_days === 'number'
    ? normalizeSyncDays(row.sync_days)
    : inferUnifiedSyncDays(plans)
  return {
    syncSpot: row.sync_spot,
    syncSwap: row.sync_swap,
    archiveAll: Boolean(row.archive_all_history),
    autoSync: true,
    syncDays,
    syncPlans: applyUnifiedSyncDays(plans, syncDays),
  }
}

function editablePlansForRow(
  row: WatchedSymbol,
  guardianPlans: GuardianPlan[],
  syncDays: number,
): WatchedSymbolSyncPlan[] {
  if (row.sync_plans && row.sync_plans.length > 0) {
    return normalizeFullSyncPlans(row.sync_plans, disabledSyncPlan)
  }
  if (isInventoryOnlyRow(row) && row.inventory_timeframes && row.inventory_timeframes.length > 0) {
    return normalizeFullSyncPlans(row.inventory_timeframes.map(timeframe => ({
      timeframe,
      enabled: true,
      bootstrap_days: normalizeSyncDays(syncDays),
      archive_mode: 'rolling',
    })), disabledSyncPlan)
  }
  return defaultEditablePlans(guardianPlans, syncDays)
}

function defaultEditablePlans(
  guardianPlans: GuardianPlan[],
  syncDays: number,
): WatchedSymbolSyncPlan[] {
  const globalPlans = guardianPlans
    .map(guardianPlanToSyncPlan)
    .filter((plan): plan is WatchedSymbolSyncPlan => !!plan)
  return applyUnifiedSyncDays(normalizeFullSyncPlans(globalPlans), normalizeSyncDays(syncDays))
}
