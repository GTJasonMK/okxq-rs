import { apiPost } from './client'

export function subscribeTicker(instId: string) {
  return apiPost('/api/market/realtime/ticker/subscribe', { inst_id: instId })
}
export function unsubscribeTicker(instId: string) {
  return apiPost('/api/market/realtime/ticker/unsubscribe', { inst_id: instId })
}
export function subscribeCandle(instId: string, timeframe: string) {
  return apiPost('/api/market/realtime/candle/subscribe', { inst_id: instId, timeframe })
}
export function unsubscribeCandle(instId: string, timeframe: string) {
  return apiPost('/api/market/realtime/candle/unsubscribe', { inst_id: instId, timeframe })
}
export function subscribeTrades(instId: string) {
  return apiPost('/api/market/realtime/trades/subscribe', { inst_id: instId })
}
export function unsubscribeTrades(instId: string) {
  return apiPost('/api/market/realtime/trades/unsubscribe', { inst_id: instId })
}
export function subscribeOrderbook(instId: string) {
  return apiPost('/api/market/realtime/orderbook/subscribe', { inst_id: instId })
}
export function unsubscribeOrderbook(instId: string) {
  return apiPost('/api/market/realtime/orderbook/unsubscribe', { inst_id: instId })
}
export function subscribeAccount(mode?: string) {
  return apiPost('/api/market/realtime/account/subscribe', mode ? { mode } : undefined)
}
export function unsubscribeAccount(mode?: string) {
  return apiPost('/api/market/realtime/account/unsubscribe', mode ? { mode } : undefined)
}
export function subscribeOrders(mode?: string) {
  return apiPost('/api/market/realtime/orders/subscribe', mode ? { mode } : undefined)
}
export function unsubscribeOrders(mode?: string) {
  return apiPost('/api/market/realtime/orders/unsubscribe', mode ? { mode } : undefined)
}
export function subscribeAlgoOrders(mode?: string) {
  return apiPost('/api/market/realtime/algo-orders/subscribe', mode ? { mode } : undefined)
}
export function unsubscribeAlgoOrders(mode?: string) {
  return apiPost('/api/market/realtime/algo-orders/unsubscribe', mode ? { mode } : undefined)
}
export function subscribeFills(mode?: string) {
  return apiPost('/api/market/realtime/fills/subscribe', mode ? { mode } : undefined)
}
export function unsubscribeFills(mode?: string) {
  return apiPost('/api/market/realtime/fills/unsubscribe', mode ? { mode } : undefined)
}
export function subscribePositions(mode?: string) {
  return apiPost('/api/market/realtime/positions/subscribe', mode ? { mode } : undefined)
}
export function unsubscribePositions(mode?: string) {
  return apiPost('/api/market/realtime/positions/unsubscribe', mode ? { mode } : undefined)
}
