import type { BacktestEquitySnapshot, Timeframe } from '@/types'
import {
  selectedSnapshotForEquityCandle,
  sortedEquityCandles,
  type EquityCandle,
} from '@/utils/strategyExecution'
import type {
  EquityHistogramMetric,
  EquityHistogramMetricOption,
  EquityHistogramPoint,
} from '../types'
import {
  finiteNumber,
  firstFiniteNumber,
  isFlatPositionSide,
} from '../format'
import { equityCandleReturnPct } from './legend'

export const EQUITY_HISTOGRAM_METRIC_OPTIONS: EquityHistogramMetricOption[] = [
  { id: 'return_pct', label: '单K收益率' },
  { id: 'drawdown_pressure_pct', label: '回撤压力' },
  { id: 'exposure_pct', label: '持仓暴露率' },
]

export function equityHistogramValues(input: {
  candles: EquityCandle[]
  snapshots: BacktestEquitySnapshot[]
  timeframe: Timeframe
  metric: EquityHistogramMetric
  sorted?: boolean
}): EquityHistogramPoint[] {
  const candles = input.sorted ? input.candles : sortedEquityCandles(input.candles)
  if (input.metric === 'return_pct') {
    return candles.map(candle => ({
      timestamp: candle.timestamp,
      value: equityCandleReturnPct(candle),
      side: '',
    }))
  }

  if (input.metric === 'drawdown_pressure_pct') {
    let peak = 0
    return candles.map(candle => {
      if (Number.isFinite(candle.close) && candle.close > peak) {
        peak = candle.close
      }
      const pressure = peak > 0 && Number.isFinite(candle.close)
        ? Math.max(0, (peak - candle.close) / peak * 100)
        : 0
      return {
        timestamp: candle.timestamp,
        value: pressure > 0 ? -pressure : 0,
        side: '',
      }
    })
  }

  return candles.map(candle => {
    const snapshot = selectedSnapshotForEquityCandle(input.snapshots, candle.timestamp, input.timeframe)
    const exposure = equityExposurePct(snapshot, candle)
    return {
      timestamp: candle.timestamp,
      value: Number.isFinite(exposure) ? exposure : 0,
      side: snapshot?.position_side ?? 'flat',
    }
  })
}

export function equityExposurePct(
  snapshot: BacktestEquitySnapshot | null | undefined,
  candle?: EquityCandle,
) {
  const equity = finiteNumber(snapshot?.equity) ?? finiteNumber(candle?.close)
  const rawNotional = firstFiniteNumber(snapshot?.position_notional, snapshot?.position_value)
  const notional = rawNotional === null
    ? (!snapshot || isFlatPositionSide(snapshot.position_side) ? 0 : Number.NaN)
    : Math.abs(rawNotional)
  if (equity === null || equity <= 0 || !Number.isFinite(notional)) return Number.NaN
  return notional / equity * 100
}
