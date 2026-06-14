import type { LiveOrder } from '@/types'
import type { StrategyTriggerKind, StrategyTriggerMarker } from '@/types/strategy-visualization'
import {
  isLiveBlockedAction,
  isLiveExitAction,
  liveExitReasonLabel,
  liveActionName,
} from '@/utils/liveOrderActions'
import {
  firstFinite,
  isValidTriggerMarker,
  sortMarkers,
  validPositive,
} from '@/utils/strategyTriggers/shared'
import {
  entryLabel,
  actionSideLabel,
} from '@/utils/strategyTriggers/labels'

export function liveOrdersToMarkers(orders: LiveOrder[]): StrategyTriggerMarker[] {
  return sortMarkers(orders.map((order, index) => {
    const timestamp = validPositive(order.timestamp) ? order.timestamp : order.created_at
    const price = firstFinite(order.avg_fill_price ?? 0, order.px ?? 0, order.arrival_mid_px ?? 0, order.arrival_ask_px ?? 0, order.arrival_bid_px ?? 0)
    const kind = liveOrderKind(order)
    return {
      id: `live-${order.id || index}-${timestamp}`,
      timestamp,
      price,
      side: order.side,
      kind,
      source: order.mode === 'live' ? 'live' : 'simulated',
      label: liveOrderLabel(order, kind),
      instId: order.inst_id,
      status: order.status,
      reason: order.action,
      detail: liveOrderDetail(order),
    } satisfies StrategyTriggerMarker
  }).filter(isValidTriggerMarker))
}

function liveOrderKind(order: LiveOrder): StrategyTriggerKind {
  const status = order.status.toLowerCase()
  const action = liveActionName(order.action)
  if (status.includes('risk') || action === 'risk_blocked') return 'risk'
  if (status.includes('fail') || status.includes('error') || !order.success) return 'blocked'
  if (status.includes('blocked') || isLiveBlockedAction(order.action)) return 'blocked'
  if (status.includes('closed') || isLiveExitAction(order.action)) {
    return 'exit'
  }
  if (status.includes('filled') || order.success) return 'entry'
  return 'pending'
}

function liveOrderLabel(order: LiveOrder, kind: StrategyTriggerKind) {
  if (kind === 'risk') return '风控'
  if (kind === 'blocked') return '拦截'
  if (kind === 'exit') return liveExitLabel(order.action)
  if (kind === 'entry') return entryLabel(order.side)
  return actionSideLabel(order.side)
}

function liveExitLabel(action: string) {
  return liveExitReasonLabel(action)
}

function liveOrderDetail(order: LiveOrder) {
  const parts = [
    order.action,
    order.status,
    liveFillSummary(order),
    order.error_message,
  ].filter(Boolean)
  return parts.join(' · ')
}

function liveFillSummary(order: LiveOrder) {
  if (!order.fill_count || order.filled_size === null) return ''
  const price = order.avg_fill_price === null ? '' : ` @ ${formatNumber(order.avg_fill_price)}`
  const fee = order.total_fee === null ? '' : ` fee ${formatNumber(order.total_fee)}${order.fee_ccy ? ` ${order.fee_ccy}` : ''}`
  return `成交 ${formatNumber(order.filled_size)}${price}${fee}`
}

function formatNumber(value: number) {
  if (!Number.isFinite(value)) return '--'
  return Math.abs(value) >= 1 ? value.toFixed(4) : value.toFixed(6)
}
