import type * as T from '@/types/backtest'
import {
  arrayRecords,
  isRecord,
  recordFrom,
  stringValue,
} from '../../normalize'
import { isValidCandle, normalizeCandle } from '../../marketNormalize'
import type { AnyRecord } from './types'
import { normalizeEquitySnapshot, isValidEquitySnapshot } from './equity'
import { idValue } from './helpers'
import { numberValue, timestampNumber } from './numbers'
import { normalizeBacktestOrders } from './orders'
import { normalizeTrade } from './trades'

export function normalizeResult(raw: AnyRecord): T.BacktestResult {
  const resultId = idValue(raw.id)
  const symbol = stringValue(raw.symbol)
  const instType = stringValue(raw.inst_type, 'SPOT') as T.BacktestResult['inst_type']
  const timeframe = stringValue(raw.timeframe, '1H') as T.BacktestResult['timeframe']
  const rawTrades = raw.trades
  const trades = arrayRecords(rawTrades).map(normalizeTrade)
  const strategyId = stringValue(raw.strategy_id)
  const strategyName = stringValue(raw.strategy_name)
  const hasTradeDetails = Array.isArray(rawTrades)
  const tradeEventsTotal = numberValue(raw.trade_events_total, hasTradeDetails ? trades.length : 0)
  const tradesTruncated = raw.trades_truncated === true
  const equityRows = arrayRecords(raw.equity_curve)
  const equitySnapshots = equityRows
    .map(normalizeEquitySnapshot)
    .filter(isValidEquitySnapshot)

  return {
    result_id: resultId,
    strategy_id: strategyId,
    strategy_name: strategyName,
    symbol,
    inst_type: instType,
    timeframe,
    days: numberValue(raw.days),
    initial_capital: numberValue(raw.initial_capital),
    final_equity: numberValue(raw.final_capital),
    total_return_pct: numberValue(raw.total_return),
    sharpe_ratio: numberValue(raw.sharpe_ratio),
    max_drawdown_pct: numberValue(raw.max_drawdown),
    win_rate_pct: numberValue(raw.win_rate),
    total_trades: numberValue(raw.total_trades),
    winning_trades: numberValue(raw.winning_trades),
    losing_trades: numberValue(raw.losing_trades),
    profit_factor: numberValue(raw.profit_factor),
    trades,
    orders: normalizeBacktestOrders(
      arrayRecords(raw.orders),
      arrayRecords(raw.fills),
      arrayRecords(raw.rejected_orders),
      { resultId, strategyId, strategyName },
    ),
    fills: arrayRecords(raw.fills),
    rejected_orders: arrayRecords(raw.rejected_orders),
    funding_events: arrayRecords(raw.funding_events),
    trade_events_total: tradeEventsTotal,
    trades_truncated: tradesTruncated,
    candles: arrayRecords(raw.candles)
      .map(candle => normalizeBacktestCandle(candle, symbol, instType, timeframe))
      .filter(isValidCandle),
    indicators: isRecord(raw.indicators) ? raw.indicators : {},
    params: normalizeResultParams(raw),
    strategy_actions: arrayRecords(raw.strategy_actions),
    strategy_diagnostics: recordFrom(raw.strategy_diagnostics),
    runtime_action_summary: recordFrom(raw.runtime_action_summary),
    execution_model: recordFrom(raw.execution_model),
    cost_model: recordFrom(raw.cost_model),
    rejected_actions: arrayRecords(raw.rejected_actions),
    strategy_context_stamp: recordFrom(raw.strategy_context_stamp),
    runtime_execution_stamp: recordFrom(raw.runtime_execution_stamp),
    backtest_result_integrity: recordFrom(raw.backtest_result_integrity),
    runtime_action_backtest: raw.runtime_action_backtest === true,
    contract_mode: raw.contract_mode === true,
    equity_curve: equitySnapshots.map(point => ({
      time: point.time,
      equity: point.equity,
    })),
    equity_snapshots: equitySnapshots,
    created_at: stringValue(raw.created_at),
  }
}

function normalizeBacktestCandle(
  candle: AnyRecord,
  symbol: string,
  instType: T.BacktestResult['inst_type'],
  timeframe: T.BacktestResult['timeframe'],
) {
  return normalizeCandle({
    ...candle,
    inst_id: symbol,
    inst_type: instType,
    timeframe,
    timestamp: timestampNumber(candle.timestamp),
    open: numberValue(candle.open),
    high: numberValue(candle.high),
    low: numberValue(candle.low),
    close: numberValue(candle.close),
    volume: numberValue(candle.volume),
    volume_ccy: numberValue(candle.volume_ccy, Number.NaN),
    volume_quote: numberValue(candle.volume_quote, Number.NaN),
  })
}

function normalizeResultParams(raw: AnyRecord): AnyRecord {
  if (isRecord(raw.params)) return raw.params
  if (typeof raw.params_json !== 'string' || raw.params_json.trim().length === 0) return {}
  try {
    const parsed: unknown = JSON.parse(raw.params_json)
    return isRecord(parsed) ? parsed : {}
  } catch {
    return {}
  }
}
