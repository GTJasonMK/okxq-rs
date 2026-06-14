import { apiGet, apiPost } from '../client'
import { arrayRecords } from '../normalize'
import {
  normalizeFill,
  normalizeOrder,
} from './normalize'
import { modeParams } from './shared'

export function fetchOrders(mode?: string) {
  return apiGet<unknown>('/api/trading/orders', modeParams(mode))
    .then(data => arrayRecords(data).map(normalizeOrder))
}

export function fetchFills(limit = 50, mode?: string) {
  return apiGet<unknown>('/api/trading/fills', { limit, ...modeParams(mode) })
    .then(data => arrayRecords(data).map(normalizeFill))
}

export function placeOrder(data: Record<string, unknown>) {
  const body: Record<string, unknown> = {
    ...data,
    sz: String(data.sz ?? ''),
    px: data.px === undefined || data.px === null ? '' : String(data.px),
  }
  if (!body.pos_side) delete body.pos_side
  if (body.reduce_only === undefined) delete body.reduce_only
  return apiPost<unknown>('/api/trading/order', body)
}

export function cancelOrder(ordId: string, instId: string, mode?: string) {
  return apiPost<unknown>('/api/trading/cancel', {
    ord_id: ordId,
    inst_id: instId,
    ...modeParams(mode),
  })
}
