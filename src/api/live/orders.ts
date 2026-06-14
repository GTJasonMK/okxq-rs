import { apiGet } from '../client'
import type * as T from '@/types/live-strategy'
import {
  arrayValue,
  booleanValue,
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  stringValue,
  timestampString,
} from '../normalize'
import { orderSide, tradingMode } from './shared'

export function fetchLiveOrders(params: { limit?: number; mode?: T.LiveStrategyStatus['mode']; run_id?: string } = {}) {
  const query: Record<string, string | number> = {}
  if (params.limit) query.limit = params.limit
  if (params.mode) query.mode = params.mode
  if (params.run_id) query.run_id = params.run_id
  return apiGet<unknown>('/api/live/orders', query).then(data => arrayValue<Record<string, unknown>>(data).map(normalizeOrder))
}

function normalizeOrder(raw: Record<string, unknown>): T.LiveOrder {
  const mode = tradingMode(raw.mode)
  const instId = stringValue(raw.inst_id)
  return {
    id: numberValue(raw.id),
    ord_id: stringValue(raw.order_id),
    client_order_id: stringValue(raw.client_order_id),
    parent_order_id: stringValue(raw.parent_order_id),
    parent_client_order_id: stringValue(raw.parent_client_order_id),
    actual_order_id: stringValue(raw.actual_order_id),
    actual_client_order_id: stringValue(raw.actual_client_order_id),
    inst_id: instId,
    symbol: stringValue(raw.symbol),
    order_type: stringValue(raw.order_type, 'market'),
    side: orderSide(raw.side, ''),
    sz: nullableNumberValue(raw.size),
    px: nullableNumberValue(raw.price),
    reference_price: nullableNumberValue(raw.reference_price),
    reference_price_source: stringValue(raw.reference_price_source),
    reference_price_missing: booleanValue(raw.reference_price_missing),
    fill_count: numberValue(raw.fill_count),
    filled_size: nullableNumberValue(raw.filled_size),
    filled_quantity: nullableNumberValue(raw.filled_quantity),
    avg_fill_price: nullableNumberValue(raw.avg_fill_price),
    fill_notional: nullableNumberValue(raw.fill_notional),
    remaining_size: nullableNumberValue(raw.remaining_size),
    total_fee: nullableNumberValue(raw.total_fee),
    fee_ccy: nullableStringValue(raw.fee_ccy),
    first_fill_ts: nullablePositiveTimestampNumber(raw.first_fill_ts),
    last_fill_ts: nullablePositiveTimestampNumber(raw.last_fill_ts),
    fill_source: stringValue(raw.fill_source),
    action: stringValue(raw.action),
    success: booleanValue(raw.success),
    status: stringValue(raw.status),
    error_message: stringValue(raw.error_message),
    mode,
    strategy_id: stringValue(raw.strategy_id),
    strategy_name: stringValue(raw.strategy_name),
    run_id: stringValue(raw.run_id),
    timestamp: nullablePositiveTimestampNumber(raw.timestamp),
    arrival_ts: nullablePositiveTimestampNumber(raw.arrival_ts),
    arrival_mid_px: nullableNumberValue(raw.arrival_mid_px),
    arrival_bid_px: nullableNumberValue(raw.arrival_bid_px),
    arrival_ask_px: nullableNumberValue(raw.arrival_ask_px),
    created_at: timestampString(raw.created_at),
  }
}

function nullableStringValue(value: unknown): string | null {
  const text = stringValue(value)
  return text ? text : null
}

function nullablePositiveTimestampNumber(value: unknown): number | null {
  const timestamp = nullableTimestampNumber(value)
  return timestamp !== null && timestamp > 0 ? timestamp : null
}
