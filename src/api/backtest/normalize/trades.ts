import type * as T from '@/types/backtest'
import {
  recordFrom,
  stringValue,
} from '../../normalize'
import type { AnyRecord } from './types'
import { orderSide, positionSide } from './helpers'
import { nullableNumberValue, numberValue, timestampNumber } from './numbers'

export function normalizeTrade(raw: AnyRecord): T.BacktestTrade {
  const metadata = recordFrom(raw.metadata)
  const side = orderSide(raw.side)
  const action = normalizeTradeAction(metadata.action ?? raw.action)
  const posSide = positionSide(metadata.pos_side ?? raw.pos_side)
  const timestamp = timestampNumber(raw.timestamp)
  const datetime = stringValue(raw.datetime) || timestampToIso(timestamp)
  const price = positivePriceOrNaN(raw.price)
  const isClose = action === 'close'
  const rawEntryPrice = positivePriceOrNull(raw.entry_price)
  const rawExitPrice = positivePriceOrNull(raw.exit_price)
  const pnl = numberValue(raw.pnl)
  const quantity = numberValue(
    raw.base_quantity,
    numberValue(raw.base_size, numberValue(raw.quantity, numberValue(raw.size))),
  )
  const exchangeQuantity = nullableNumberValue(raw.exchange_quantity ?? raw.size)
  const value = numberValue(raw.value)
  const equity = nullableNumberValue(metadata.equity ?? raw.equity)
  return {
    symbol: stringValue(raw.symbol) || stringValue(metadata.symbol) || undefined,
    timestamp,
    datetime,
    entry_time: isClose ? '' : datetime,
    exit_time: isClose ? datetime : '',
    side,
    action,
    pos_side: posSide,
    price,
    entry_price: isClose ? rawEntryPrice : rawEntryPrice ?? positivePriceOrNull(price),
    exit_price: isClose ? rawExitPrice ?? positivePriceOrNull(price) : rawExitPrice,
    quantity,
    base_quantity: quantity,
    exchange_quantity: exchangeQuantity ?? undefined,
    value,
    commission: numberValue(raw.commission),
    pnl,
    pnl_pct: numberValue(raw.pnl_pct),
    funding: numberValue(metadata.funding ?? raw.funding),
    equity: equity ?? 0,
    reason: stringValue(raw.reason),
  }
}

function normalizeTradeAction(value: unknown): string {
  const action = stringValue(value).trim().toLowerCase()
  if (action === 'funding') return 'funding'
  if (
    action === 'open'
    || action === 'open_position'
    || action === 'entry'
    || action === 'buy_open'
    || action === 'sell_open'
  ) return 'open'
  if (
    action === 'close'
    || action === 'close_position'
    || action === 'place_risk_order'
    || action === 'risk'
    || action === 'exit'
    || action === 'buy_close'
    || action === 'sell_close'
  ) return 'close'
  return ''
}

function timestampToIso(timestamp: number) {
  return timestamp > 0 ? new Date(timestamp).toISOString() : ''
}

function positivePriceOrNaN(value: unknown): number {
  return positivePriceOrNull(value) ?? Number.NaN
}

function positivePriceOrNull(value: unknown): number | null {
  const parsed = nullableNumberValue(value)
  return parsed !== null && parsed > 0 ? parsed : null
}
