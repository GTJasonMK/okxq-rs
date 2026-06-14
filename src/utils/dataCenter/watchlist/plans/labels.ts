import type {
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import {
  inferUnifiedSyncDays,
  syncPlanSummary,
} from '@/utils/syncPlans'
import { isInventoryOnlyRow } from '@/utils/dataCenter/watchlist/guards'
import { effectivePlansForRow } from './effective'

export function rowPlanSummary(row: WatchedSymbol, enabledPlans: WatchedSymbolSyncPlan[]) {
  if (isInventoryOnlyRow(row)) {
    const timeframes = row.inventory_timeframes ?? []
    if (timeframes.length > 0) return `${timeframes.join('/')} · 库内数据 · 未接管规则`
    return '库内数据 · 未接管规则'
  }
  const plans = effectivePlansForRow(row, enabledPlans)
  if (row.archive_all_history && plans.length > 0) {
    return `${plans.map(plan => plan.timeframe).join('/')} · 全量 · 1m底座派生`
  }
  const days = row.sync_days ?? inferUnifiedSyncDays(plans)
  if (plans.length === 0) return `${syncPlanSummary(plans)} · 1m底座派生`
  return `${plans.map(plan => plan.timeframe).join('/')} · ${days}天 · 1m底座派生`
}

export function ruleModeLabel(row: WatchedSymbol) {
  if (isInventoryOnlyRow(row)) return '库内未关注'
  const base = row.sync_plans && row.sync_plans.length > 0 ? '自定义规则' : '全局规则'
  return row.archive_all_history ? `${base} · 强制全量` : base
}

export function planPolicyLabel(plan: WatchedSymbolSyncPlan, forceFull?: boolean) {
  if (isFullArchivePlan(plan, forceFull)) return '全量'
  return `${plan.bootstrap_days}天`
}

export function isFullArchivePlan(plan: WatchedSymbolSyncPlan, forceFull?: boolean) {
  return forceFull === true || plan.archive_mode === 'full'
}
