import type { Timeframe } from '@/types'
import { DAY_MS } from '@/utils/marketView/constants'

export function candleBucketStart(timestamp: number, timeframe: Timeframe) {
  if (timeframe === '1M') {
    const date = new Date(timestamp)
    return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1, 0, 0, 0)
  }
  if (timeframe === '1W') {
    const date = new Date(timestamp)
    const day = date.getUTCDay() || 7
    return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() - day + 1, 0, 0, 0)
  }
  const ms = timeframeToMs(timeframe)
  return timestamp - (timestamp % ms)
}

export function timeframeToMs(timeframe: Timeframe) {
  const minute = 60_000
  const values: Record<Timeframe, number> = {
    '1m': minute,
    '3m': 3 * minute,
    '5m': 5 * minute,
    '15m': 15 * minute,
    '30m': 30 * minute,
    '1H': 60 * minute,
    '2H': 2 * 60 * minute,
    '4H': 4 * 60 * minute,
    '6H': 6 * 60 * minute,
    '12H': 12 * 60 * minute,
    '1D': DAY_MS,
    '1W': 7 * DAY_MS,
    '1M': 30 * DAY_MS,
  }
  return values[timeframe] || minute
}
