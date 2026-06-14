import type {
  Orderbook,
  RecentTrade,
  Ticker,
} from '@/types/market'
import {
  isRecord,
  nullableNumberValue,
  numberValue,
  stringValue as textValue,
  timestampNumber as timestampValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeInstId,
  normalizeInstType,
} from './core'

export function normalizeTicker(raw: unknown): Ticker | null {
  const item = isRecord(raw) ? raw : {}
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  const last = positiveNumberValue(item.last)
  if (last === null) return null
  const open24h = positiveNumberValue(item.open24h) ?? last
  const rawChangePercent = numberValue(item.change24h, Number.NaN)
  const change24h = Number.isFinite(rawChangePercent)
    ? rawChangePercent
    : (open24h > 0 ? ((last - open24h) / open24h) * 100 : 0)
  return {
    inst_id: normalizeInstId(rawInstId, instType),
    inst_type: instType,
    last,
    ask: positiveNumberValue(item.ask) ?? last,
    bid: positiveNumberValue(item.bid) ?? last,
    open24h,
    high24h: positiveNumberValue(item.high24h) ?? last,
    low24h: positiveNumberValue(item.low24h) ?? last,
    vol24h: numberValue(item.vol24h),
    change24h,
    ts: timestampValue(item.ts),
  }
}

export function normalizeOrderbook(raw: unknown): Orderbook {
  const item = isRecord(raw) ? raw : {}
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  return {
    inst_id: normalizeInstId(rawInstId, instType),
    bids: normalizeBookSide(item.bids),
    asks: normalizeBookSide(item.asks),
    ts: timestampValue(item.ts),
  }
}

export function normalizeRecentTrade(raw: unknown): RecentTrade | null {
  const item = isRecord(raw) ? raw : {}
  const ts = timestampValue(item.ts, Date.now())
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  const price = positiveNumberValue(item.price)
  const size = positiveNumberValue(item.size)
  if (price === null || size === null) return null
  return {
    inst_id: normalizeInstId(rawInstId, instType),
    trade_id: textValue(item.trade_id),
    price,
    size,
    side: orderSide(item.side),
    ts,
  }
}

function normalizeBookSide(value: unknown): Array<{ price: number; size: number; count: number }> {
  if (!Array.isArray(value)) return []
  return value.flatMap((row) => {
    const item = isRecord(row) ? row : {}
    const price = positiveNumberValue(item.price)
    const size = positiveNumberValue(item.size)
    if (price === null || size === null) return []
    return [{
      price,
      size,
      count: numberValue(item.count, 1),
    }]
  })
}

function orderSide(value: unknown): RecentTrade['side'] {
  if (value === 'buy' || value === 'sell') return value
  return 'buy'
}

function positiveNumberValue(value: unknown): number | null {
  const parsed = nullableNumberValue(value)
  return parsed !== null && parsed > 0 ? parsed : null
}
