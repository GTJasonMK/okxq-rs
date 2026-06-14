import type { BacktestEquitySnapshot, Timeframe } from '@/types'
import { candleBucketStart, timeframeToMs } from '@/utils/marketView'
import type { EquityCandle } from '@/utils/strategyExecution/types'
import { sortedEquitySnapshots } from '@/utils/strategyExecution/sorting'

export function buildEquityCandles(
  snapshots: BacktestEquitySnapshot[],
  timeframe: Timeframe,
): EquityCandle[] {
  const buckets = new Map<number, EquityCandle>()
  const rows = sortedEquitySnapshots(snapshots)

  for (const row of rows) {
    if (!Number.isFinite(row.time) || row.time <= 0 || !Number.isFinite(row.equity)) continue
    const timestamp = candleBucketStart(row.time, timeframe)
    const existing = buckets.get(timestamp)
    if (!existing) {
      buckets.set(timestamp, {
        timestamp,
        open: row.equity,
        high: row.equity,
        low: row.equity,
        close: row.equity,
        volume: 1,
        snapshot_count: 1,
      })
      continue
    }
    existing.high = Math.max(existing.high, row.equity)
    existing.low = Math.min(existing.low, row.equity)
    existing.close = row.equity
    existing.volume += 1
    existing.snapshot_count += 1
  }

  return Array.from(buckets.values()).sort((left, right) => left.timestamp - right.timestamp)
}

export function equityCandleEnd(candleTimestamp: number, timeframe: Timeframe) {
  if (timeframe === '1M') {
    const date = new Date(candleTimestamp)
    return Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 1, 0, 0, 0)
  }
  return candleTimestamp + timeframeToMs(timeframe)
}
