import type * as T from '@/types/trading'
import { okxNullableNumberValue as numericValue } from '../okxPayload'
import {
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  recordFrom,
  stringValue,
  timestampNumber as timestampValue,
} from '../normalize'

export function normalizeContractAccountConfig(raw: unknown): T.ContractAccountConfig {
  const item = recordFrom(raw)
  const posMode = stringValue(item.posMode)
  return {
    pos_mode: posMode === 'long_short_mode' ? 'long_short_mode' : 'net_mode',
    raw: item,
  }
}

export function normalizeContractLeverage(item: Record<string, unknown>): T.ContractLeverageInfo {
  return {
    inst_id: stringValue(item.inst_id ?? item.instId),
    mgn_mode: normalizeMarginMode(item.mgn_mode ?? item.mgnMode),
    pos_side: normalizePositionSideText(item.pos_side ?? item.posSide),
    lever: numericValue(item.lever) ?? 0,
  }
}

export function normalizePosition(item: Record<string, unknown>): T.Position {
  return {
    inst_id: stringValue(item.inst_id),
    inst_type: instTypeValue(item.inst_type),
    pos_side: positionSideValue(item.pos_side),
    pos: nullableNumberValue(item.pos),
    mgn_mode: normalizeMarginMode(item.mgn_mode ?? item.mgnMode),
    avg_px: nullableNumberValue(item.avg_px),
    upl: nullableNumberValue(item.upl),
    upl_ratio: nullableNumberValue(item.upl_ratio),
    lever: nullableNumberValue(item.lever),
    margin: nullableNumberValue(item.margin),
    mark_px: nullableNumberValue(item.mark_px),
  }
}

export function normalizeOrder(item: Record<string, unknown>): T.Order {
  return {
    ord_id: stringValue(item.ord_id),
    inst_id: stringValue(item.inst_id),
    side: orderSideValue(item.side),
    ord_type: normalizeOrderType(item.ord_type),
    sz: nullableNumberValue(item.sz),
    px: nullableNumberValue(item.px),
    state: stringValue(item.state, 'live'),
    fill_sz: nullableNumberValue(item.fill_sz),
    fill_px: nullableNumberValue(item.fill_px),
    avg_px: nullableNumberValue(item.avg_px),
    pnl: nullableNumberValue(item.pnl),
    ctime: nullableTimestampNumber(item.c_time),
  }
}

export function normalizeFill(item: Record<string, unknown>): T.Fill {
  return {
    fill_id: stringValue(item.trade_id),
    inst_id: stringValue(item.inst_id),
    ord_id: stringValue(item.ord_id),
    side: orderSideValue(item.side),
    fill_px: nullableNumberValue(item.fill_px),
    fill_sz: nullableNumberValue(item.fill_sz),
    fee: nullableNumberValue(item.fee),
    fee_ccy: stringValue(item.fee_ccy),
    fill_time: nullableTimestampNumber(item.ts),
  }
}

export function normalizeLocalFill(item: Record<string, unknown>): T.LocalFill | null {
  const price = nullableNumberValue(item.fill_px)
  const rawQuantity = nullableNumberValue(item.fill_sz)
  if (!isPositiveFinite(price) || !isNonZeroFinite(rawQuantity)) return null
  const quantity = Math.abs(rawQuantity)
  return {
    id: stringValue(item.id),
    inst_id: stringValue(item.inst_id),
    ccy: stringValue(item.ccy, baseCurrency(item.inst_id)),
    side: orderSideValue(item.side, rawQuantity < 0 ? 'sell' : 'buy'),
    quantity,
    price,
    fee: nullableNumberValue(item.fee),
    total_cost: price * quantity,
    fill_time: normalizeTimeString(item.ts),
  }
}

export function normalizeLocalFillsSyncResult(raw: unknown): T.LocalFillsSyncResult {
  const item = recordFrom(raw)
  return {
    mode: stringValue(item.mode),
    inst_type: stringValue(item.inst_type),
    inst_id: stringValue(item.inst_id),
    fetched: numberValue(item.fetched),
    stored: numberValue(item.stored),
    skipped_missing_trade_id: numberValue(item.skipped_missing_trade_id),
    arrival_matched: numberValue(item.arrival_matched),
    note: stringValue(item.note),
  }
}

export function normalizeCostBasis(item: Record<string, unknown>): T.CostBasis | null {
  const totalQuantity = nullableNumberValue(item.total_qty)
  const totalCost = nullableNumberValue(item.total_cost)
  if (!isPositiveFinite(totalQuantity) || !isFiniteNumber(totalCost)) return null
  const avgPrice = nullableNumberValue(item.avg_cost) ?? totalCost / totalQuantity
  if (!isFiniteNumber(avgPrice)) return null
  return {
    ccy: stringValue(item.ccy),
    total_quantity: totalQuantity,
    total_cost: totalCost,
    avg_price: avgPrice,
    unrealized_pnl: 0,
  }
}

export function normalizePerformance(item: Record<string, unknown>): T.TradePerformance | null {
  const totalTrades = nullableNumberValue(item.total_trades)
  if (!isNonNegativeFinite(totalTrades)) return null
  return {
    inst_id: stringValue(item.inst_id, totalTrades > 0 ? 'ALL' : ''),
    total_trades: totalTrades,
    win_rate: nullableNumberValue(item.win_rate),
    total_pnl: nullableNumberValue(item.total_pnl),
    profit_factor: nullableNumberValue(item.profit_factor),
    largest_win: nullableNumberValue(item.largest_win),
    largest_loss: nullableNumberValue(item.largest_loss),
  }
}

export function normalizeRiskControl(raw: unknown): T.RiskControlConfig {
  const item = recordFrom(raw)
  return {
    enabled: typeof item.enabled === 'boolean' ? item.enabled : true,
    max_single_loss_ratio: numberValue(item.max_single_loss_ratio, 0.02),
    max_position_pct: numberValue(item.max_position_pct, 0.2),
    max_order_value: numberValue(item.max_order_value),
  }
}

function normalizeOrderType(value: unknown): T.Order['ord_type'] {
  const normalized = stringValue(value, 'limit').trim().toLowerCase()
  return normalized === 'market' ? 'market' : 'limit'
}

function baseCurrency(value: unknown): string {
  const symbol = stringValue(value)
  const [base] = symbol.split('-')
  return base || ''
}

function normalizeTimeString(value: unknown): string {
  const timestamp = timestampValue(value, Number.NaN)
  if (!Number.isFinite(timestamp) || timestamp <= 0) return ''
  return timestampToIso(timestamp)
}

function timestampToIso(timestamp: number): string {
  const milliseconds = timestamp < 1_000_000_000_000 ? timestamp * 1000 : timestamp
  const date = new Date(milliseconds)
  return Number.isFinite(date.getTime()) ? date.toISOString() : ''
}

function isPositiveFinite(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function isNonZeroFinite(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) > 0
}

function isNonNegativeFinite(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function isFiniteNumber(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function orderSideValue(value: unknown, defaultValue: T.Order['side'] = 'buy'): T.Order['side'] {
  const side = stringValue(value).trim().toLowerCase()
  if (side === 'buy' || side === 'sell') return side
  return defaultValue
}

function positionSideValue(value: unknown): T.Position['pos_side'] {
  const side = stringValue(value).trim().toLowerCase()
  if (side === 'long' || side === 'short') return side
  return ''
}

function normalizePositionSideText(value: unknown): T.ContractLeverageInfo['pos_side'] {
  const side = stringValue(value).trim().toLowerCase()
  if (side === 'long' || side === 'short') return side
  if (side === 'net') return 'net'
  return ''
}

function normalizeMarginMode(value: unknown): T.MarginMode {
  const mode = stringValue(value, 'cross').trim().toLowerCase()
  if (mode === 'cash' || mode === 'isolated') return mode
  return 'cross'
}

function instTypeValue(value: unknown): T.Position['inst_type'] {
  const instType = stringValue(value, 'SPOT').trim().toUpperCase()
  if (instType === 'SPOT' || instType === 'SWAP' || instType === 'FUTURES') return instType
  return 'SPOT'
}
