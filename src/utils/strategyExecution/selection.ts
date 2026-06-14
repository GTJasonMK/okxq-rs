import type { BacktestEquitySnapshot, BacktestTrade, Candle, Timeframe } from '@/types'
import { candleBucketStart } from '@/utils/marketView'
import { equityCandleEnd } from '@/utils/strategyExecution/candles'
import {
  lowerBoundByTimestamp,
  sortedCandles,
  sortedEquitySnapshots,
  sortedTradeEvents,
  upperBoundByTimestamp,
} from '@/utils/strategyExecution/sorting'

export function selectedSnapshotAtOrBefore(
  snapshots: BacktestEquitySnapshot[],
  timestamp: number,
): BacktestEquitySnapshot | null {
  return selectedSnapshotAtOrBeforeSorted(sortedEquitySnapshots(snapshots), timestamp)
}

export function selectedSnapshotAtOrBeforeSorted(
  rows: readonly BacktestEquitySnapshot[],
  timestamp: number,
): BacktestEquitySnapshot | null {
  if (rows.length === 0) return null
  if (!Number.isFinite(timestamp) || timestamp <= 0) return rows[rows.length - 1]

  const index = upperBoundByTimestamp(rows, timestamp, row => row.time) - 1
  return rows[Math.max(0, index)] ?? null
}

export function selectedCandleAtOrBefore(candles: Candle[], timestamp: number): Candle | null {
  return selectedCandleAtOrBeforeSorted(sortedCandles(candles), timestamp)
}

export function selectedCandleAtOrBeforeSorted(
  rows: readonly Candle[],
  timestamp: number,
): Candle | null {
  if (rows.length === 0) return null
  if (!Number.isFinite(timestamp) || timestamp <= 0) return rows[rows.length - 1]

  const index = upperBoundByTimestamp(rows, timestamp, row => row.timestamp) - 1
  return rows[Math.max(0, index)] ?? null
}

export function nearbyTradeEvents(
  trades: BacktestTrade[],
  timestamp: number,
  limit = 8,
): BacktestTrade[] {
  return nearbyTradeEventsSorted(sortedTradeEvents(trades), timestamp, limit)
}

export function nearbyTradeEventsSorted(
  rows: readonly BacktestTrade[],
  timestamp: number,
  limit = 8,
): BacktestTrade[] {
  if (limit <= 0 || rows.length === 0) return []
  if (!Number.isFinite(timestamp) || timestamp <= 0) return rows.slice(-limit).reverse()

  const nearby: BacktestTrade[] = []
  let left = lowerBoundByTimestamp(rows, timestamp, row => row.timestamp) - 1
  let right = left + 1

  while (nearby.length < limit && (left >= 0 || right < rows.length)) {
    const leftDistance = left >= 0 ? Math.abs(rows[left].timestamp - timestamp) : Number.POSITIVE_INFINITY
    const rightDistance = right < rows.length ? Math.abs(rows[right].timestamp - timestamp) : Number.POSITIVE_INFINITY
    if (leftDistance <= rightDistance) {
      nearby.push(rows[left])
      left -= 1
    } else {
      nearby.push(rows[right])
      right += 1
    }
  }

  return nearby.sort((leftRow, rightRow) => leftRow.timestamp - rightRow.timestamp)
}

export function selectedSnapshotForEquityCandle(
  rows: readonly BacktestEquitySnapshot[],
  candleTimestamp: number,
  timeframe: Timeframe,
): BacktestEquitySnapshot | null {
  if (rows.length === 0 || !Number.isFinite(candleTimestamp) || candleTimestamp <= 0) return null

  const start = candleBucketStart(candleTimestamp, timeframe)
  const end = equityCandleEnd(start, timeframe)
  const index = upperBoundByTimestamp(rows, end - 1, row => row.time) - 1
  const candidate = rows[index]
  if (candidate && candidate.time >= start) return candidate
  return selectedSnapshotAtOrBeforeSorted(rows, start)
}

export function selectedPositionSnapshotForEquityCandle(
  rows: readonly BacktestEquitySnapshot[],
  candleTimestamp: number,
  timeframe: Timeframe,
): BacktestEquitySnapshot | null {
  const closingSnapshot = selectedSnapshotForEquityCandle(rows, candleTimestamp, timeframe)
  if (snapshotHasOpenPosition(closingSnapshot)) return closingSnapshot
  if (rows.length === 0 || !Number.isFinite(candleTimestamp) || candleTimestamp <= 0) {
    return closingSnapshot
  }

  const start = candleBucketStart(candleTimestamp, timeframe)
  const end = equityCandleEnd(start, timeframe)
  const startIndex = lowerBoundByTimestamp(rows, start, row => row.time)
  const endIndex = lowerBoundByTimestamp(rows, end, row => row.time)
  for (let index = endIndex - 1; index >= startIndex; index -= 1) {
    const candidate = rows[index]
    if (snapshotHasOpenPosition(candidate)) return candidate
  }
  return closingSnapshot
}

export function tradeEventsForEquityCandle(
  rows: readonly BacktestTrade[],
  candleTimestamp: number,
  timeframe: Timeframe,
  limit = 8,
): BacktestTrade[] {
  if (limit <= 0 || rows.length === 0 || !Number.isFinite(candleTimestamp) || candleTimestamp <= 0) return []

  const start = candleBucketStart(candleTimestamp, timeframe)
  const end = equityCandleEnd(start, timeframe)
  const startIndex = lowerBoundByTimestamp(rows, start, row => row.timestamp)
  const endIndex = lowerBoundByTimestamp(rows, end, row => row.timestamp)
  return rows.slice(startIndex, endIndex).slice(0, limit)
}

function snapshotHasOpenPosition(snapshot: BacktestEquitySnapshot | null): boolean {
  if (!snapshot) return false
  if (snapshot.positions && snapshot.positions.length > 0) return true
  const side = snapshot.position_side
  if (side === 'flat' || side === '') return false
  return hasFiniteExposure(snapshot.position_notional)
    || hasFiniteExposure(snapshot.position_value)
    || Number.isFinite(snapshot.unrealized_pnl)
}

function hasFiniteExposure(value: number | null | undefined): boolean {
  return Number.isFinite(value) && Math.abs(Number(value)) > 0
}
