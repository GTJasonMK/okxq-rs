import type * as T from '@/types/backtest'
import {
  isRecord,
  recordFrom,
  stringValue,
} from '../../normalize'
import type { AnyRecord } from './types'
import { positionSide } from './helpers'
import {
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  timestampNumber,
} from './numbers'

export function normalizeEquitySnapshot(point: AnyRecord): T.BacktestEquitySnapshot {
  const positionValue = nullableNumberValue(point.position_value)
  return {
    time: timestampNumber(point.timestamp),
    equity: numberValue(point.equity),
    cash: nullableNumberValue(point.cash),
    position_value: positionValue,
    position_notional: nullableNumberValue(point.position_notional) ?? positionValue,
    unrealized_pnl: nullableNumberValue(point.unrealized_pnl),
    position_side: positionSide(point.position_side),
    leverage: numberValue(point.leverage, 1),
    positions: normalizeEquityPositions(point.positions),
  }
}

export function isValidEquitySnapshot(point: T.BacktestEquitySnapshot) {
  return Number.isFinite(point.time) && point.time > 0 && Number.isFinite(point.equity)
}

function normalizeEquityPositions(raw: unknown): T.BacktestPositionSnapshot[] | undefined {
  const rows = Array.isArray(raw)
    ? raw
        .map((value, index) => ({ symbolKey: `${index}`, value }))
        .filter((entry): entry is { symbolKey: string, value: AnyRecord } => isRecord(entry.value))
    : Object.entries(recordFrom(raw))
        .map(([symbolKey, value]) => ({ symbolKey, value }))
        .filter((entry): entry is { symbolKey: string, value: AnyRecord } => isRecord(entry.value))
  const positions = rows
    .map(({ symbolKey, value }) => normalizeEquityPosition(symbolKey, value))
    .filter(position => position.symbol.length > 0)
    .sort((left, right) => left.symbol.localeCompare(right.symbol))
  return positions.length > 0 ? positions : undefined
}

function normalizeEquityPosition(symbolKey: string, raw: AnyRecord): T.BacktestPositionSnapshot {
  const symbol = stringValue(raw.symbol) || stringValue(raw.instId) || symbolKey
  const side = positionSide(raw.side ?? raw.posSide)
  const entryPrice = nullableNumberValue(raw.entry_price ?? raw.avgPx)
  const quantity = nullableNumberValue(
    raw.base_quantity
      ?? raw.base_size
      ?? raw.basePos
      ?? raw.base_position_size
      ?? raw.quantity
      ?? raw.pos,
  )
  const exchangeQuantity = nullableNumberValue(raw.exchange_quantity ?? raw.pos)
  const entryNotional = nullableNumberValue(raw.entry_notional)
    ?? (entryPrice !== null && quantity !== null ? entryPrice * quantity : null)
  const notional = nullableNumberValue(raw.notional)
    ?? nullableNumberValue(raw.position_notional)
    ?? nullableNumberValue(raw.notionalUsd)
    ?? entryNotional
  return {
    symbol,
    side,
    inst_type: stringValue(raw.inst_type) || stringValue(raw.instType),
    timeframe: stringValue(raw.timeframe) as T.BacktestPositionSnapshot['timeframe'],
    entry_price: entryPrice,
    quantity,
    exchange_quantity: exchangeQuantity,
    entry_timestamp: nullableTimestampNumber(raw.entry_timestamp),
    entry_notional: entryNotional,
    entry_reason: stringValue(raw.entry_reason),
    reason: stringValue(raw.reason),
    stop_loss: nullableNumberValue(raw.stop_loss),
    take_profit: nullableNumberValue(raw.take_profit),
    planned_exit_time: nullableTimestampNumber(raw.planned_exit_time),
    planned_exit_reason: stringValue(raw.planned_exit_reason),
    planned_hold_bars: nullableNumberValue(raw.planned_hold_bars),
    mark_price: nullableNumberValue(raw.mark_price ?? raw.markPx),
    mark_price_source: stringValue(raw.mark_price_source) || undefined,
    mark_price_missing: raw.mark_price_missing === true,
    notional,
    position_notional: nullableNumberValue(raw.position_notional) ?? notional,
    unrealized_pnl: nullableNumberValue(raw.unrealized_pnl ?? raw.upl),
    unrealized_pnl_pct: nullableNumberValue(raw.unrealized_pnl_pct),
  }
}
