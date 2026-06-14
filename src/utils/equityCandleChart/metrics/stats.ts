import type { BacktestEquitySnapshot } from '@/types'
import {
  sortedEquityCandles,
  sortedEquitySnapshots,
  type EquityCandle,
} from '@/utils/strategyExecution'
import type {
  EquityExtreme,
  EquityStats,
  RangeSummary,
} from '../types'
import {
  formatMoneyValue,
  formatShortDate,
} from '../format'

export function equityRangeSummary(stats: EquityStats): RangeSummary {
  return {
    max: `${formatMoneyValue(stats.max.value)} · ${formatShortDate(stats.max.timestamp)}`,
    min: `${formatMoneyValue(stats.min.value)} · ${formatShortDate(stats.min.timestamp)}`,
    range: formatMoneyValue(stats.range),
    drawdown: `${stats.maxDrawdownPct.toFixed(2)}%`,
  }
}

export function equityStats(
  snapshots: BacktestEquitySnapshot[],
  candles: EquityCandle[],
  options: { sorted?: boolean } = {},
): EquityStats | null {
  const points = equityPoints(snapshots, candles, options)
  if (points.length === 0) return null

  let max: EquityExtreme | null = null
  let min: EquityExtreme | null = null
  let peak: EquityExtreme | null = null
  let maxDrawdownPct = 0

  for (const point of points) {
    if (!max || point.value > max.value) {
      max = point
    }
    if (!min || point.value < min.value) {
      min = point
    }
    if (!peak || point.value > peak.value) {
      peak = point
    }
    if (peak.value > 0) {
      maxDrawdownPct = Math.max(maxDrawdownPct, (peak.value - point.value) / peak.value * 100)
    }
  }

  if (!max || !min) return null
  return {
    max,
    min,
    range: max.value - min.value,
    maxDrawdownPct,
  }
}

export function equityPoints(
  snapshots: BacktestEquitySnapshot[],
  candles: EquityCandle[],
  options: { sorted?: boolean } = {},
): EquityExtreme[] {
  const snapshotRows = options.sorted ? snapshots : sortedEquitySnapshots(snapshots)
  const snapshotPoints = snapshotRows
    .map(row => ({ timestamp: row.time, value: row.equity }))
  if (snapshotPoints.length > 0) return snapshotPoints

  const candlePoints: EquityExtreme[] = []
  const candleRows = options.sorted ? candles : sortedEquityCandles(candles)
  for (const candle of candleRows) {
    if (!Number.isFinite(candle.timestamp) || candle.timestamp <= 0) continue
    for (const value of [candle.open, candle.high, candle.low, candle.close]) {
      if (Number.isFinite(value)) {
        candlePoints.push({ timestamp: candle.timestamp, value })
      }
    }
  }
  return candlePoints
}
