import { apiGet } from '../client'
import type * as T from '@/types/live-strategy'
import {
  arrayValue,
  booleanValue,
  numberValue,
  stringValue,
  timestampNumber,
  timestampString,
} from '../normalize'
import { tradingMode, positionSide } from './shared'

export function fetchLiveEquity(params: { limit?: number; mode?: T.LiveStrategyStatus['mode']; run_id?: string } = {}) {
  const query: Record<string, string | number> = {}
  if (params.limit) query.limit = params.limit
  if (params.mode) query.mode = params.mode
  if (params.run_id) query.run_id = params.run_id
  return apiGet<unknown>('/api/live/equity', query).then(normalizeEquityHistory)
}

function normalizeEquityHistory(raw: unknown): T.LiveEquityHistory {
  const item = raw && typeof raw === 'object' && !Array.isArray(raw)
    ? raw as Record<string, unknown>
    : {}
  return {
    run_id: stringValue(item.run_id),
    mode: tradingMode(item.mode),
    count: numberValue(item.count),
    snapshots: arrayValue<Record<string, unknown>>(item.snapshots)
      .map(normalizeEquitySnapshot)
      .filter((snapshot): snapshot is T.LiveEquitySnapshot => snapshot !== null),
    daily: arrayValue<Record<string, unknown>>(item.daily)
      .map(normalizeDailySummary)
      .filter((summary): summary is T.LiveEquityDailySummary => summary !== null),
    pnl_available: booleanValue(item.pnl_available, true),
    source: stringValue(item.source),
  }
}

function normalizeEquitySnapshot(raw: Record<string, unknown>): T.LiveEquitySnapshot | null {
  const price = nullableLiveEquityNumber(raw.price)
  const entryPrice = nullableLiveEquityNumber(raw.entry_price)
  const quantity = nullableLiveEquityNumber(raw.quantity)
  const initialCapital = liveEquityNumber(raw.initial_capital)
  const dayStartEquity = liveEquityNumber(raw.day_start_equity)
  const equity = liveEquityNumber(raw.equity)
  const realizedPnl = nullableLiveEquityNumber(raw.realized_pnl)
  const unrealizedPnl = nullableLiveEquityNumber(raw.unrealized_pnl)
  const totalPnl = nullableLiveEquityNumber(raw.total_pnl)
  const totalPnlPct = nullableLiveEquityNumber(raw.total_pnl_pct)
  const todayPnl = nullableLiveEquityNumber(raw.today_pnl)
  const todayPnlPct = nullableLiveEquityNumber(raw.today_pnl_pct)
  if (
    initialCapital === null
    || dayStartEquity === null
    || equity === null
  ) return null
  return {
    id: numberValue(raw.id),
    run_id: stringValue(raw.run_id),
    strategy_id: stringValue(raw.strategy_id),
    strategy_name: stringValue(raw.strategy_name),
    symbol: stringValue(raw.symbol),
    inst_id: stringValue(raw.inst_id),
    timeframe: stringValue(raw.timeframe, '1H') as T.LiveEquitySnapshot['timeframe'],
    inst_type: stringValue(raw.inst_type, 'SPOT') as T.LiveEquitySnapshot['inst_type'],
    mode: tradingMode(raw.mode),
    timestamp: timestampNumber(raw.timestamp),
    time: stringValue(raw.time),
    trading_day: stringValue(raw.trading_day),
    price,
    position_side: positionSide(raw.position_side, ''),
    entry_price: entryPrice,
    quantity,
    initial_capital: initialCapital,
    day_start_equity: dayStartEquity,
    equity,
    realized_pnl: realizedPnl,
    unrealized_pnl: unrealizedPnl,
    total_pnl: totalPnl,
    total_pnl_pct: totalPnlPct,
    today_pnl: todayPnl,
    today_pnl_pct: todayPnlPct,
    created_at: timestampString(raw.created_at),
    pnl_available: booleanValue(raw.pnl_available, true),
    source: stringValue(raw.source),
  }
}

function normalizeDailySummary(raw: Record<string, unknown>): T.LiveEquityDailySummary | null {
  const snapshotCount = liveEquityNumber(raw.snapshot_count)
  const firstEquity = liveEquityNumber(raw.first_equity)
  const lastEquity = liveEquityNumber(raw.last_equity)
  const dayStartEquity = liveEquityNumber(raw.day_start_equity)
  const todayPnl = nullableLiveEquityNumber(raw.today_pnl)
  const todayPnlPct = nullableLiveEquityNumber(raw.today_pnl_pct)
  const totalPnl = nullableLiveEquityNumber(raw.total_pnl)
  const totalPnlPct = nullableLiveEquityNumber(raw.total_pnl_pct)
  const realizedPnl = nullableLiveEquityNumber(raw.realized_pnl)
  const unrealizedPnl = nullableLiveEquityNumber(raw.unrealized_pnl)
  if (
    snapshotCount === null
    || firstEquity === null
    || lastEquity === null
    || dayStartEquity === null
  ) return null
  return {
    trading_day: stringValue(raw.trading_day),
    start_timestamp: timestampNumber(raw.start_timestamp),
    end_timestamp: timestampNumber(raw.end_timestamp),
    start_time: stringValue(raw.start_time),
    end_time: stringValue(raw.end_time),
    snapshot_count: snapshotCount,
    first_equity: firstEquity,
    last_equity: lastEquity,
    day_start_equity: dayStartEquity,
    today_pnl: todayPnl,
    today_pnl_pct: todayPnlPct,
    total_pnl: totalPnl,
    total_pnl_pct: totalPnlPct,
    realized_pnl: realizedPnl,
    unrealized_pnl: unrealizedPnl,
    pnl_available: booleanValue(raw.pnl_available, true),
  }
}

function liveEquityNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function nullableLiveEquityNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}
