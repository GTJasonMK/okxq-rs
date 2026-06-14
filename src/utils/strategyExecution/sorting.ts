import type { BacktestEquitySnapshot, BacktestTrade, Candle } from '@/types'
import type { EquityCandle } from '@/utils/strategyExecution/types'

export function sortedEquitySnapshots(snapshots: readonly BacktestEquitySnapshot[]): BacktestEquitySnapshot[] {
  return sortedValidRows(
    snapshots,
    row => row.time,
    row => Number.isFinite(row.equity),
  )
}

export function sortedCandles(candles: readonly Candle[]): Candle[] {
  return sortedValidRows(candles, row => row.timestamp)
}

export function sortedEquityCandles(
  candles: readonly EquityCandle[],
  options: { copy?: boolean } = {},
): EquityCandle[] {
  return sortedValidRows(candles, row => row.timestamp, undefined, options.copy ?? true)
}

export function sortedTradeEvents(trades: readonly BacktestTrade[]): BacktestTrade[] {
  return sortedValidRows(trades, row => row.timestamp)
}

export function lowerBoundByTimestamp<T>(
  rows: readonly T[],
  timestamp: number,
  timestampOf: (row: T) => number,
) {
  return boundByTimestamp(rows, timestampOf, (value) => value < timestamp)
}

export function upperBoundByTimestamp<T>(
  rows: readonly T[],
  timestamp: number,
  timestampOf: (row: T) => number,
) {
  return boundByTimestamp(rows, timestampOf, (value) => value <= timestamp)
}

function boundByTimestamp<T>(
  rows: readonly T[],
  timestampOf: (row: T) => number,
  moveRight: (value: number) => boolean,
) {
  let low = 0
  let high = rows.length
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (moveRight(timestampOf(rows[middle]))) {
      low = middle + 1
    } else {
      high = middle
    }
  }
  return low
}

function sortedValidRows<T>(
  rows: readonly T[],
  timestampOf: (row: T) => number,
  isValid: (row: T) => boolean = () => true,
  copySorted = true,
): T[] {
  let previous = Number.NEGATIVE_INFINITY
  for (const row of rows) {
    const timestamp = timestampOf(row)
    if (!Number.isFinite(timestamp) || timestamp <= 0 || !isValid(row) || timestamp < previous) {
      return rows
        .filter(item => {
          const itemTimestamp = timestampOf(item)
          return Number.isFinite(itemTimestamp) && itemTimestamp > 0 && isValid(item)
        })
        .sort((left, right) => timestampOf(left) - timestampOf(right))
    }
    previous = timestamp
  }
  return copySorted ? rows.slice() : rows as T[]
}
