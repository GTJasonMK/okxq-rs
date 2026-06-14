import type {
  BacktestTrade,
  LiveOrder,
  OrderSide,
} from '@/types'
import {
  firstPositiveNumber,
  positiveNumber,
} from '@/utils/liveStrategyEquityChart/numbers'

export function liveOrdersForEquityChart(orders: readonly LiveOrder[]): BacktestTrade[] {
  return orders
    .map(liveOrderForChart)
    .filter((row): row is BacktestTrade => Boolean(row))
    .sort((left, right) => left.timestamp - right.timestamp)
}

function liveOrderForChart(order: LiveOrder): BacktestTrade | null {
  const timestamp = firstPositiveNumber(order.timestamp, order.arrival_ts, order.created_at)
  if (timestamp === null) return null

  const side = normalizedOrderSide(order)
  const action = normalizedOrderAction(order)
  const posSide = normalizedOrderPositionSide(side, action)
  const price = firstPositiveNumber(order.avg_fill_price, order.px, order.arrival_mid_px, order.arrival_bid_px, order.arrival_ask_px)
  const quantity = positiveNumber(order.filled_size) ?? positiveNumber(order.sz)
  if (price === null || quantity === null) return null
  const value = price * quantity
  const timeText = formatChartEventTime(timestamp)

  return {
    symbol: order.inst_id || order.symbol || '',
    timestamp,
    datetime: timeText,
    entry_time: action === 'open' ? timeText : '',
    exit_time: action === 'close' ? timeText : '',
    side,
    action,
    pos_side: posSide,
    price,
    entry_price: action === 'open' ? price : null,
    exit_price: action === 'close' ? price : null,
    quantity,
    value,
    commission: Math.abs(order.total_fee ?? 0),
    pnl: 0,
    pnl_pct: 0,
    funding: 0,
    equity: 0,
    reason: order.action || order.status || order.error_message || 'live_order',
  }
}

function normalizedOrderAction(order: LiveOrder) {
  if (order.action.trim().toLowerCase() === 'close_position') {
    return 'close'
  }
  return 'open'
}

function normalizedOrderSide(order: LiveOrder): OrderSide {
  const side = order.side.trim().toLowerCase()
  if (side === 'buy' || side === 'sell') return side
  return 'buy'
}

function normalizedOrderPositionSide(side: OrderSide, action: string) {
  if (action === 'close') return side === 'buy' ? 'short' : 'long'
  return side === 'sell' ? 'short' : 'long'
}

function formatChartEventTime(timestamp: number) {
  return Number.isFinite(timestamp) && timestamp > 0
    ? new Date(timestamp).toISOString()
    : ''
}
