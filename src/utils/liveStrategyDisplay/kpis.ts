import {
  compareEquitySnapshotsByTime,
  compareOrdersByLatest,
} from '@/utils/liveStrategyCore'
import type { LiveDecisionActionSummary, LiveOrder, LiveEquityHistory } from '@/types'
import {
  formatMoneyCompact,
  formatSignedMoney,
  formatSignedPercentPoint,
  pnlKind,
} from './format'
import {
  formatOrderAction,
  liveOrderCounts,
  orderStatusLabel,
} from './orders'
import type {
  BuildLiveStrategyKpisInput,
  DecisionKpiInput,
  LiveStrategyKpi,
} from './types'

export function latestLiveEquitySnapshot(history: LiveEquityHistory | null) {
  let latest: LiveEquityHistory['snapshots'][number] | null = null
  for (const snapshot of history?.snapshots ?? []) {
    if (!latest || compareEquitySnapshotsByTime(snapshot, latest) > 0) {
      latest = snapshot
    }
  }
  return latest
}

export function decisionKpi(input: DecisionKpiInput): LiveStrategyKpi {
  const diagnostics = input.diagnostics
  if (!diagnostics) {
    return {
      label: '策略决策',
      value: input.decisionDiagnosticsLoading ? '评估中' : '未评估',
      detail: input.decisionDiagnosticsLoading
        ? input.decisionDiagnosticsScopeText
        : input.autoDecisionDiagnosticsEnabled ? '等待下一次自动诊断' : '点击刷新评估当前配置',
      kind: input.decisionDiagnosticsLoading ? 'neutral' : 'warning',
    }
  }
  const summary = diagnostics.action_summary
  const verdict = diagnostics.execution_decision?.verdict || ''
  if (verdict === 'ready') {
    return {
      label: '策略决策',
      value: '可执行',
      detail: actionSummaryText(summary),
      kind: 'ready',
    }
  }
  if (verdict === 'blocked') {
    return {
      label: '策略决策',
      value: '已阻断',
      detail: diagnostics.execution_decision?.summary || diagnostics.summary || actionSummaryText(summary),
      kind: 'blocked',
    }
  }
  if (summary.total > 0) {
    return {
      label: '策略决策',
      value: `${summary.total} 个动作`,
      detail: actionSummaryText(summary),
      kind: executableActionCount(summary) > 0 ? 'ready' : 'neutral',
    }
  }
  return {
    label: '策略决策',
    value: '等待',
    detail: diagnostics.summary || '策略当前未返回动作',
    kind: 'neutral',
  }
}

export function buildLiveStrategyKpis(input: BuildLiveStrategyKpisInput): LiveStrategyKpi[] {
  const equitySnapshot = latestLiveEquitySnapshot(input.equityHistory)
  const equity = equitySnapshot?.equity ?? 0
  const totalPnl = equitySnapshot?.total_pnl ?? null
  const totalPnlPct = equitySnapshot?.total_pnl_pct ?? null
  const todayPnl = equitySnapshot?.today_pnl ?? null
  const todayPnlPct = equitySnapshot?.today_pnl_pct ?? null
  const unrealizedPnl = equitySnapshot?.unrealized_pnl ?? null
  const pnlAvailable = equitySnapshot !== null
    && input.equityHistory?.pnl_available !== false
    && equitySnapshot.pnl_available !== false
  const latestOrder = latestLiveOrder(input.orders)
  const orderCounts = liveOrderCounts(input.orders)
  const hasTotalPnl = totalPnl !== null && totalPnl !== 0
  const hasEquity = equity > 0 || hasTotalPnl
  const equityKpi: LiveStrategyKpi = pnlAvailable ? {
    label: '权益/收益',
    value: hasEquity && totalPnlPct !== null ? formatSignedPercentPoint(totalPnlPct) : '暂无',
    detail: hasEquity
      ? `权益 ${formatMoneyCompact(equity)} · ${totalPnl === null ? '收益未知' : formatSignedMoney(totalPnl)}`
      : '等待 OKX 账户权益',
    kind: pnlKind(totalPnl ?? 0),
  } : {
    label: '权益/收益',
    value: equity > 0 ? formatMoneyCompact(equity) : '暂无',
    detail: equitySnapshot
      ? `OKX 账户权益 · 未实现 ${unrealizedPnl === null ? '未知' : formatSignedMoney(unrealizedPnl)}`
      : '等待 OKX 账户权益',
    kind: pnlKind(unrealizedPnl ?? 0),
  }
  const todayKpi: LiveStrategyKpi = pnlAvailable ? {
    label: '今日收益',
    value: (equity > 0 || todayPnl !== null) && todayPnlPct !== null ? formatSignedPercentPoint(todayPnlPct) : '暂无',
    detail: (equity > 0 || todayPnl !== null) && todayPnl !== null ? formatSignedMoney(todayPnl) : '今日暂无快照',
    kind: pnlKind(todayPnl ?? 0),
  } : {
    label: '今日收益',
    value: '未提供',
    detail: equitySnapshot ? 'OKX balance 未提供日内收益基准' : '等待 OKX 账户权益',
    kind: 'neutral',
  }
  return [
    decisionKpi(input),
    equityKpi,
    todayKpi,
    {
      label: '订单',
      value: `${input.orders.length} 条`,
      detail: latestOrder
        ? `最新 ${formatOrderAction(latestOrder)} · ${orderStatusLabel(latestOrder.status)} · 失败 ${orderCounts.failed}`
        : '策略启动、平仓或风控拦截后显示',
      kind: orderCounts.failed > 0 ? 'negative' : orderCounts.blocked > 0 ? 'warning' : 'neutral',
    },
  ]
}

function actionSummaryText(summary: LiveDecisionActionSummary) {
  const parts = [
    ['开仓', summary.open_position],
    ['平仓', summary.close_position],
    ['保护单', summary.place_risk_order],
    ['撤单', summary.cancel_order],
    ['改单', summary.modify_order],
    ['等待', summary.hold],
  ]
    .filter(([, count]) => Number(count) > 0)
    .map(([label, count]) => `${label} ${count}`)
  return parts.length > 0 ? parts.join('，') : '策略当前未返回动作'
}

function executableActionCount(summary: LiveDecisionActionSummary) {
  return summary.open_position
    + summary.close_position
    + summary.place_risk_order
    + summary.cancel_order
    + summary.modify_order
}

export function latestLiveOrder(orders: LiveOrder[]) {
  let latest: LiveOrder | null = null
  for (const order of orders) {
    if (!latest || compareOrdersByLatest(order, latest) < 0) {
      latest = order
    }
  }
  return latest
}
