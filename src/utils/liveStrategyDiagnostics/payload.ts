import { inferInstTypeFromId } from '@/api/marketNormalize'
import { stableJson } from '@/utils/liveStrategyCore'
import type {
	  LiveStrategyDiagnosticTarget,
	  RealtimeDecisionCandle,
	} from '@/utils/liveStrategyDiagnostics/types'

export function latestRealtimeDiagnosticCandle(
  candle: RealtimeDecisionCandle | null,
  target: LiveStrategyDiagnosticTarget,
) {
  if (!candle) return null
  if (candle.inst_id !== target.symbol || candle.timeframe !== target.timeframe) return null
  const volumeCcy = nonNegativeNumber(candle.volume_ccy)
  const volumeQuote = nonNegativeNumber(candle.volume_quote)
  const confirm = normalizedConfirm(candle.confirm)
  if (volumeCcy === null || volumeQuote === null || confirm === null) return null
  return {
    inst_id: candle.inst_id,
    inst_type: candle.inst_type,
    timeframe: candle.timeframe,
    timestamp: candle.timestamp,
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
    volume: candle.volume,
    volume_ccy: volumeCcy,
    volume_quote: volumeQuote,
    confirm,
  }
}

export function diagnosticsRefreshRequestKey(
  targetKey: string,
  latestCandle: ReturnType<typeof latestRealtimeDiagnosticCandle>,
) {
  return stableJson({
    target: targetKey,
    latest: latestCandle
      ? {
          timestamp: latestCandle.timestamp,
          open: latestCandle.open,
          high: latestCandle.high,
          low: latestCandle.low,
          close: latestCandle.close,
          volume: latestCandle.volume,
          volume_ccy: latestCandle.volume_ccy,
          volume_quote: latestCandle.volume_quote,
          confirm: latestCandle.confirm,
        }
      : null,
  })
}

export function decisionDiagnosticsPayload(
  target: LiveStrategyDiagnosticTarget,
  latestCandle: ReturnType<typeof latestRealtimeDiagnosticCandle>,
) {
  return diagnosticPayload(target, latestCandle)
}

function diagnosticPayload(
  target: LiveStrategyDiagnosticTarget,
  latestCandle: ReturnType<typeof latestRealtimeDiagnosticCandle>,
): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    strategy_id: target.strategy_id,
    symbol: target.symbol,
    inst_type: inferInstTypeFromId(target.symbol),
    timeframe: target.timeframe,
    initial_capital: target.initial_capital,
    position_size: target.position_size,
    stop_loss: target.stop_loss,
    take_profit: target.take_profit,
    mode: target.mode,
    params: target.params,
    fresh: false,
  }
  if (latestCandle) payload.latest_candle = latestCandle
  return payload
}

function nonNegativeNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function normalizedConfirm(value: unknown): '0' | '1' | null {
  return value === '0' || value === '1' ? value : null
}
