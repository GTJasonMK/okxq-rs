import {
  isLiveBlockedAction,
  isLiveExitAction,
  liveActionLabel,
  liveActionName,
} from '@/utils/liveOrderActions'
import type { LiveOrder } from '@/types'

export type LiveOrderPositionSide = 'long' | 'short' | ''

const POSITION_ENTRY_ACTION_NAMES = new Set(['open_position'])
const PENDING_ORDER_STATUSES = new Set([
  'submitting',
  'submit_unknown',
  'submitted',
  'pending',
  'open',
  'live',
  'cancel_requested',
  'modify_requested',
  'algo_submitting',
  'algo_submit_unknown',
  'algo_submitted',
  'algo_live',
  'algo_cancel_requested',
  'algo_modify_requested',
])

export function liveOrderCounts(orders: LiveOrder[]) {
  return orders.reduce((summary, order) => {
    if (orderSummaryKind(order) === 'blocked') summary.blocked += 1
    if (isFailedLiveOrder(order)) summary.failed += 1
    return summary
  }, { blocked: 0, failed: 0 })
}

function orderSummaryKind(order: LiveOrder) {
  if (isLiveOrderBlocked(order)) return 'blocked'
  if (isLiveOrderPending(order)) return 'pending'
  if (!order.success) return 'blocked'
  if (isLiveOrderExit(order)) return 'exit'
  return 'filled'
}

export function formatOrderAction(order: LiveOrder) {
  const actionLabel = liveActionLabel(order.action)
  if (isLiveOrderBlocked(order)) {
    if (isLiveRiskOrder(order)) return '风控拦截'
    return actionLabel === '--' ? '风控拦截' : actionLabel
  }
  if (isLiveOrderExit(order)) return liveOrderExitActionLabel(order)
  if (isLivePositionEntryAction(order.action)) return liveOrderEntryActionLabel(order)
  if (actionLabel !== '--') return actionLabel
  if (isContractLiveOrder(order)) return liveOrderEntryActionLabel(order)
  const sideLabel = liveOrderSpotSideLabel(order.side)
  return sideLabel === '--' ? actionLabel : sideLabel
}

export function orderStatusLabel(status: string) {
  const map: Record<string, string> = {
    filled: '已成交',
    live: '挂单中',
    submitting: '提交中',
    submit_unknown: '提交结果待确认',
    submitted: '已提交',
    submit_failed: '提交失败',
    rejected: '已拒绝',
    reject: '已拒绝',
    cancel_requested: '撤单已请求',
    modify_requested: '改单已请求',
    canceled: '已撤单',
    cancelled: '已撤单',
    partially_filled: '部分成交',
    partial: '部分成交',
    algo_submitted: '保护单已提交',
    algo_submit_unknown: '保护单提交待确认',
    algo_live: '保护单生效中',
    algo_cancel_requested: '保护单撤销已请求',
    algo_modify_requested: '保护单改单已请求',
    algo_effective: '保护单已触发',
    algo_partially_effective: '保护单部分触发',
    algo_canceled: '保护单已撤销',
    algo_cancelled: '保护单已撤销',
    algo_failed: '保护单失败',
    risk_blocked: '风控拦截',
    blocked: '已拦截',
  }
  return map[status] || status || '--'
}

export function orderStatusText(order: LiveOrder) {
  return order.status.toLowerCase()
}

export function isContractLiveOrder(order: LiveOrder) {
  return order.inst_id.includes('-SWAP') || order.inst_id.includes('-FUTURES')
}

export function isLiveOrderExit(order: LiveOrder) {
  return orderStatusText(order).includes('closed') || isLiveExitAction(order.action)
}

export function isLiveOrderBlocked(order: LiveOrder) {
  const status = orderStatusText(order)
  return status.includes('blocked') || status.includes('risk') || isLiveBlockedAction(order.action)
}

export function isLiveOrderPending(order: LiveOrder) {
  return PENDING_ORDER_STATUSES.has(orderStatusText(order))
}

export function isLiveRiskOrder(order: LiveOrder) {
  return orderStatusText(order).includes('risk') || liveActionName(order.action) === 'risk_blocked'
}

export function isFailedLiveOrder(order: LiveOrder) {
  const status = orderStatusText(order)
  if (isLiveOrderPending(order)) return false
  return !order.success || status.includes('fail') || status.includes('error')
}

export function isLivePositionEntryAction(action: string) {
  return POSITION_ENTRY_ACTION_NAMES.has(liveActionName(action))
}

export function liveOrderPositionSide(
  order: LiveOrder,
  exit = isLiveOrderExit(order),
): LiveOrderPositionSide {
  if (!isContractLiveOrder(order)) return ''
  if (exit) {
    if (order.side === 'buy') return 'short'
    if (order.side === 'sell') return 'long'
    return ''
  }
  if (order.side === 'buy') return 'long'
  if (order.side === 'sell') return 'short'
  return ''
}

export function liveOrderHistoryActionLabel(
  order: LiveOrder,
  exit = isLiveOrderExit(order),
  positionSide = liveOrderPositionSide(order, exit),
) {
  if (exit) {
    if (positionSide === 'short') return '平空'
    if (positionSide === 'long') return '平多'
    return '平仓'
  }
  if (positionSide === 'long') return orderStatusText(order) === 'live' ? '挂多' : '开多'
  if (positionSide === 'short') return orderStatusText(order) === 'live' ? '挂空' : '开空'
  return liveOrderSpotSideLabel(order.side)
}

export function liveOrderEntryActionLabel(order: LiveOrder) {
  if (!isContractLiveOrder(order)) return liveOrderSpotSideLabel(order.side)
  if (order.side === 'sell') return orderStatusText(order) === 'live' ? '挂空' : '开空'
  if (order.side === 'buy') return orderStatusText(order) === 'live' ? '挂多' : '开多'
  return '--'
}

export function liveOrderExitActionLabel(order: LiveOrder) {
  if (!isContractLiveOrder(order)) {
    if (order.side === 'sell') return '卖出'
    if (order.side === 'buy') return '买入'
    return '平仓'
  }
  if (order.side === 'buy') return '平空'
  if (order.side === 'sell') return '平多'
  return '平仓'
}

export function liveOrderSpotSideLabel(side: string) {
  if (side === 'buy') return '买入'
  if (side === 'sell') return '卖出'
  return side || '--'
}

export function liveOrderHistoryStatusClass(order: LiveOrder) {
  const status = orderStatusText(order)
  if (isFailedLiveOrder(order)) return 'failed'
  if (isLiveOrderExit(order)) return 'closed'
  if (status.includes('filled')) return 'filled'
  if (isLiveOrderPending(order)) return 'pending'
  return 'neutral'
}
