import type { Orderbook, RecentTrade, Ticker } from '@/types'
import {
  normalizeOrderbook,
  normalizeRecentTrade,
  normalizeTicker,
} from './marketNormalize'
import {
  okxNumberValue as numberValue,
  okxNullableNumberValue as nullableNumberValue,
  okxPositiveNumberValue as positiveNumberValue,
  okxStringValue as stringValue,
  okxTimestampValue as timestampValue,
} from './okxPayload'
import { isRecord } from './normalize'

export function normalizeRealtimeTicker(raw: unknown): Ticker | null {
  const item = isRecord(raw) ? raw : {}
  const change24h = nullableNumberValue(item.change24h)
  return normalizeTicker({
    inst_id: stringValue(item.inst_id ?? item.instId),
    inst_type: item.inst_type ?? item.instType,
    last: positiveNumberValue(item.last ?? item.lastPx),
    ask: positiveNumberValue(item.ask ?? item.askPx),
    bid: positiveNumberValue(item.bid ?? item.bidPx),
    open24h: positiveNumberValue(item.open24h ?? item.open24hPx),
    high24h: positiveNumberValue(item.high24h ?? item.high24hPx),
    low24h: positiveNumberValue(item.low24h ?? item.low24hPx),
    vol24h: numberValue(item.vol24h ?? item.volCcy24h ?? item.vol, 0),
    ...(change24h !== null ? { change24h } : {}),
    ts: timestampValue(item.ts),
  })
}

export function normalizeRealtimeOrderbook(raw: unknown): Orderbook {
  const item = isRecord(raw) ? raw : {}
  return normalizeOrderbook({
    inst_id: stringValue(item.inst_id ?? item.instId),
    inst_type: item.inst_type ?? item.instType,
    bids: normalizeRealtimeBookSide(item.bids),
    asks: normalizeRealtimeBookSide(item.asks),
    ts: timestampValue(item.ts),
  })
}

export function normalizeRealtimeTrade(raw: unknown): RecentTrade | null {
  const item = isRecord(raw) ? raw : {}
  return normalizeRecentTrade({
    inst_id: stringValue(item.inst_id ?? item.instId),
    inst_type: item.inst_type ?? item.instType,
    trade_id: stringValue(item.trade_id ?? item.tradeId),
    price: positiveNumberValue(item.price ?? item.px),
    size: positiveNumberValue(item.size ?? item.sz),
    side: item.side,
    ts: timestampValue(item.ts, Date.now()),
  })
}

function normalizeRealtimeBookSide(value: unknown): Orderbook['bids'] {
  if (!Array.isArray(value)) return []
  return value.flatMap(normalizeRealtimeBookLevel)
}

function normalizeRealtimeBookLevel(row: unknown): Orderbook['bids'] {
  const [priceValue, sizeValue, countValue] = Array.isArray(row)
    ? [row[0], row[1], row[3] ?? row[2]]
    : realtimeBookLevelValues(row)
  const price = positiveNumberValue(priceValue)
  const size = positiveNumberValue(sizeValue)
  if (price === null || size === null) return []
  return [{
    price,
    size,
    count: Math.max(0, Math.round(numberValue(countValue, 1))),
  }]
}

function realtimeBookLevelValues(row: unknown) {
  const item = isRecord(row) ? row : {}
  return [item.price ?? item.px, item.size ?? item.sz, item.count ?? item.ordCnt]
}
