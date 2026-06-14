import type { LiveOrder } from '@/types/live-strategy'
import { booleanValue, stringValue } from '../../normalize'
import type { AnyRecord } from './types'
import { nullableNumberValue, nullableTimestampNumber } from './numbers'

interface OrderNormalizeContext {
  resultId: string
  strategyId: string
  strategyName: string
}

interface FillAggregate {
  count: number
  filledSize: number
  notional: number
  priceSizeValue: number
  fee: number
  firstTs: number | null
  lastTs: number | null
}

export function normalizeBacktestOrders(
  rawOrders: AnyRecord[],
  rawFills: AnyRecord[],
  rawRejectedOrders: AnyRecord[],
  context: OrderNormalizeContext,
): LiveOrder[] {
  const fillAggregates = aggregateFills(rawFills)
  const rows = rawOrders.map((order, index) =>
    normalizeBacktestOrder(order, index + 1, fillAggregates.get(orderKey(order)), context)
  )
  const knownKeys = new Set(rows.map(order => liveOrderKey(order)).filter(Boolean))
  for (const rejected of rawRejectedOrders) {
    const key = orderKey(rejected)
    if (key && knownKeys.has(key)) continue
    rows.push(normalizeBacktestOrder(rejected, rows.length + 1, fillAggregates.get(key), context))
    if (key) knownKeys.add(key)
  }
  return rows
}

function normalizeBacktestOrder(
  raw: AnyRecord,
  id: number,
  aggregate: FillAggregate | undefined,
  context: OrderNormalizeContext,
): LiveOrder {
  const instId = normalizedInstId(raw.inst_id ?? raw.symbol)
  const submittedSize = nullableNumberValue(raw.size)
  const rawFilledSize = nullableNumberValue(raw.filled_size)
  const filledSize = positiveOrNull(aggregate?.filledSize) ?? rawFilledSize
  const rawNotional = nullableNumberValue(raw.fill_notional ?? raw.value)
  const fillNotional = positiveOrNull(aggregate?.notional) ?? rawNotional
  const avgFillPrice = averageFillPrice(aggregate) ?? positiveOrNull(nullableNumberValue(raw.avg_fill_price))
  const orderPrice = positiveOrNull(nullableNumberValue(raw.price ?? raw.trigger_price))
  const timestamp = positiveTimestamp(
    raw.timestamp ?? raw.updated_ts ?? raw.submitted_ts ?? raw.action_timestamp,
  )
  const status = stringValue(raw.status, raw.success === false ? 'rejected' : '')
  const success = raw.success === false ? false : !isRejectedStatus(status)
  return {
    id,
    ord_id: stringValue(raw.order_id),
    client_order_id: stringValue(raw.client_order_id),
    parent_order_id: '',
    parent_client_order_id: '',
    actual_order_id: '',
    actual_client_order_id: '',
    inst_id: instId,
    symbol: normalizedInstId(raw.symbol) || instId,
    order_type: stringValue(raw.order_type, 'market'),
    side: orderSide(raw.side),
    sz: submittedSize,
    px: orderPrice,
    reference_price: positiveOrNull(nullableNumberValue(raw.reference_price)),
    reference_price_source: stringValue(raw.reference_price_source),
    reference_price_missing: booleanValue(raw.reference_price_missing),
    fill_count: aggregate?.count ?? (filledSize !== null && filledSize > 0 ? 1 : 0),
    filled_size: filledSize,
    filled_quantity: filledSize,
    avg_fill_price: avgFillPrice,
    fill_notional: fillNotional,
    remaining_size: nullableNumberValue(raw.remaining_size),
    total_fee: aggregate?.fee ?? nullableNumberValue(raw.total_fee ?? raw.commission ?? raw.fee),
    fee_ccy: null,
    first_fill_ts: aggregate?.firstTs ?? null,
    last_fill_ts: aggregate?.lastTs ?? null,
    fill_source: aggregate ? 'historical_live_backtest' : '',
    action: stringValue(raw.action),
    success,
    status,
    error_message: stringValue(raw.error_message ?? raw.reason),
    mode: 'simulated',
    strategy_id: context.strategyId,
    strategy_name: context.strategyName,
    run_id: context.resultId,
    timestamp,
    arrival_ts: null,
    arrival_mid_px: null,
    arrival_bid_px: null,
    arrival_ask_px: null,
    created_at: timestamp ?? 0,
  }
}

function aggregateFills(fills: AnyRecord[]) {
  const aggregates = new Map<string, FillAggregate>()
  for (const fill of fills) {
    const key = orderKey(fill)
    if (!key) continue
    const price = nullableNumberValue(fill.price)
    const size = nullableNumberValue(fill.size ?? fill.filled_size ?? fill.quantity)
    const notional = nullableNumberValue(fill.value)
      ?? (price !== null && size !== null ? price * size : null)
    const fee = nullableNumberValue(fill.commission ?? fill.fee)
    const timestamp = positiveTimestamp(fill.timestamp)
    const aggregate = aggregates.get(key) ?? {
      count: 0,
      filledSize: 0,
      notional: 0,
      priceSizeValue: 0,
      fee: 0,
      firstTs: null,
      lastTs: null,
    }
    aggregate.count += 1
    aggregate.filledSize += size ?? 0
    aggregate.notional += notional ?? 0
    aggregate.priceSizeValue += price !== null && size !== null ? price * size : 0
    aggregate.fee += fee ?? 0
    if (timestamp !== null) {
      aggregate.firstTs = aggregate.firstTs === null ? timestamp : Math.min(aggregate.firstTs, timestamp)
      aggregate.lastTs = aggregate.lastTs === null ? timestamp : Math.max(aggregate.lastTs, timestamp)
    }
    aggregates.set(key, aggregate)
  }
  return aggregates
}

function orderKey(raw: AnyRecord) {
  return stringValue(raw.order_id) || stringValue(raw.client_order_id)
}

function liveOrderKey(order: LiveOrder) {
  return order.ord_id || order.client_order_id
}

function averageFillPrice(aggregate: FillAggregate | undefined) {
  if (!aggregate || aggregate.filledSize <= 0 || aggregate.priceSizeValue <= 0) return null
  return aggregate.priceSizeValue / aggregate.filledSize
}

function positiveOrNull(value: number | null | undefined) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null
}

function positiveTimestamp(value: unknown) {
  const timestamp = nullableTimestampNumber(value)
  return timestamp !== null && timestamp > 0 ? timestamp : null
}

function normalizedInstId(value: unknown) {
  return stringValue(value).trim().toUpperCase()
}

function orderSide(value: unknown) {
  const side = stringValue(value).trim().toLowerCase()
  return side === 'buy' || side === 'sell' ? side : ''
}

function isRejectedStatus(status: string) {
  const normalized = status.trim().toLowerCase()
  return normalized === 'rejected' || normalized === 'reject' || normalized.includes('fail')
}
