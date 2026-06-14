import type {
  LiveExecutionPlan,
  LiveOrder,
  LiveEquityHistory,
  LiveStrategyStatus,
  TradingMode,
} from '@/types'
import { dailySummariesFromSnapshots } from '@/utils/liveStrategyCore/equity'
import {
  formatRuntimeRefreshTime,
  modeLabel,
  shortRunId,
} from '@/utils/liveStrategyCore/labels'
import type {
  DetailDataScopeTextInput,
  LiveDataScope,
} from '@/utils/liveStrategyCore/types'

export function scopedLiveOrders(orders: LiveOrder[], scope: LiveDataScope): LiveOrder[] {
  return orders.filter(order => {
    if (order.mode !== scope.mode) return false
    if (scope.runId && order.run_id !== scope.runId) return false
    return true
  })
}

export function scopedLiveExecutionPlans(plans: LiveExecutionPlan[], scope: LiveDataScope): LiveExecutionPlan[] {
  return plans.filter(plan => {
    if (plan.mode !== scope.mode) return false
    if (!scope.runId) return true
    return plan.entry_run_id === scope.runId || plan.exit_run_id === scope.runId
  })
}

export function scopedLiveEquityHistory(
  history: LiveEquityHistory | null,
  scope: LiveDataScope,
): LiveEquityHistory | null {
  if (!history || history.mode !== scope.mode) return null
  if (scope.runId && history.run_id !== scope.runId) return null
  const snapshots = history.snapshots.filter(snapshot =>
    snapshot.mode === scope.mode &&
    (!scope.runId || snapshot.run_id === scope.runId)
  )
  return {
    ...history,
    count: snapshots.length,
    snapshots,
    daily: dailySummariesFromSnapshots(snapshots),
  }
}

export function detailDataScopeText(input: DetailDataScopeTextInput): string {
  const current = input.status
  const mode = modeLabel(input.mode)
  const hiddenOrders = input.hiddenOrderCount > 0
    ? ` · 已隐藏 ${input.hiddenOrderCount} 条非当前范围历史记录`
    : ''
  const hiddenEquity = input.hiddenEquityByScope ? ' · 权益不属于当前范围已隐藏' : ''
  if (current?.running && current.run_id) {
    return `${mode} · 当前运行 ${shortRunId(current.run_id)} · 历史仓位/权益仅显示本次运行${hiddenOrders}${hiddenEquity}`
  }
  if (current?.run_id) {
    return `${mode} · 上次运行 ${shortRunId(current.run_id)} · 历史仓位/权益固定显示该 run${hiddenOrders}${hiddenEquity}`
  }
  const equityRun = input.scopedEquityHistory?.run_id
    ? ` · 权益 ${shortRunId(input.scopedEquityHistory.run_id)}`
    : ''
  return `${mode} · 未运行时显示最近记录${equityRun}${hiddenOrders}${hiddenEquity}`
}

export function runtimeRefreshNoticeText(error: string | null, lastRuntimeRefreshAt: number) {
  if (!error) return ''
  const refreshedAt = lastRuntimeRefreshAt > 0
    ? formatRuntimeRefreshTime(lastRuntimeRefreshAt)
    : '尚无成功刷新'
  return `运行状态刷新失败，当前页面显示上一次成功刷新结果（${refreshedAt}）。错误：${error}`
}

export function liveRuntimeDataScope(status: LiveStrategyStatus | null, launchMode: TradingMode): LiveDataScope {
  return {
    mode: status?.mode ?? launchMode,
    runId: status?.run_id || '',
  }
}
