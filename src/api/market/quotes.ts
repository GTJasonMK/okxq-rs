import { apiGet } from '../client'
import { arrayRecords, arrayValue } from '../normalize'
import {
  isValidCandle,
  normalizeCandle,
  normalizeOrderbook,
  normalizeRecentTrade,
  normalizeTicker,
} from '../marketNormalize'

export function fetchCandles(instId: string, opts?: Record<string, string | number | boolean>) {
  const params: Record<string, string | number | boolean> = { limit: 100, ...opts }
  return apiGet<unknown>(`/api/market/candles/${encodeURIComponent(instId)}`, params)
    .then(data => arrayRecords(data).map(normalizeCandle).filter(isValidCandle))
}

export function fetchTicker(instId: string, instType?: string) {
  const params: Record<string, string | number> = {}
  if (instType) params.inst_type = instType
  return apiGet<unknown>(`/api/market/ticker/${encodeURIComponent(instId)}`, params)
    .then(data => normalizeTicker(data))
}

export function fetchTickers() {
  return apiGet<unknown>('/api/market/tickers')
    .then(data => arrayValue(data).map(item => normalizeTicker(item)).filter(isPresent))
}

export function fetchOrderbook(instId: string, size = 20, instType?: string) {
  const params: Record<string, string | number> = { size }
  if (instType) params.inst_type = instType
  return apiGet<unknown>(`/api/market/orderbook/${encodeURIComponent(instId)}`, params)
    .then(data => normalizeOrderbook(data))
}

export function fetchRecentTrades(instId: string, limit = 50, instType?: string) {
  const params: Record<string, string | number> = { limit }
  if (instType) params.inst_type = instType
  return apiGet<unknown>(`/api/market/trades/${encodeURIComponent(instId)}`, params)
    .then(data => arrayValue(data).map(normalizeRecentTrade).filter(isPresent))
}

function isPresent<TValue>(value: TValue | null): value is TValue {
  return value !== null
}
