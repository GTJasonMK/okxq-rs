import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import { isValidCandle } from '@/api/marketNormalize'
import { DAY_MS, MAX_CHART_CANDLE_ROWS } from '@/utils/marketView/constants'
import { candleBucketStart, timeframeToMs } from './time'
import { lowerBoundCandleTimestamp, sortedValidCandles } from './sorted'

export function mergeCandles(stored: Candle[], realtimeDerived: Candle[]) {
  const validStored = sortedValidCandles(stored)
  const validRealtimeDerived = sortedValidCandles(realtimeDerived)
  if (validRealtimeDerived.length === 0) return validStored.slice()
  return mergeSortedCandles(validStored, validRealtimeDerived).slice(-MAX_CHART_CANDLE_ROWS)
}

export function aggregateCandles(candles: Candle[], timeframe: Timeframe): Candle[] {
  const rows = sortedValidCandles(candles)
  if (timeframe === '1m') return rows.slice()
  return aggregateSortedCandles(rows, timeframe)
}

export function aggregateSortedCandlesForRange(
  candles: Candle[],
  timeframe: Timeframe,
  rangeDays: CandleRangeDays,
): Candle[] {
  if (candles.length === 0) return []
  if (timeframe === '1m') return candles.slice()
  const latest = candles[candles.length - 1]?.timestamp ?? 0
  if (!Number.isFinite(latest) || latest <= 0) return aggregateSortedCandles(candles, timeframe)
  const timeframeMs = Math.max(timeframeToMs(timeframe), 1)
  const cutoff = latest - rangeDays * DAY_MS + timeframeMs
  const bucketCutoff = candleBucketStart(cutoff, timeframe)
  const startIndex = lowerBoundCandleTimestamp(
    candles,
    Number.isFinite(bucketCutoff) ? bucketCutoff : cutoff,
  )
  return aggregateSortedCandles(candles.slice(startIndex), timeframe)
}

function aggregateSortedCandles(candles: Candle[], timeframe: Timeframe): Candle[] {
  const buckets = new Map<number, Candle>()
  for (const candle of candles) {
    if (!isValidCandle(candle)) continue
    const bucketStart = candleBucketStart(candle.timestamp, timeframe)
    if (!Number.isFinite(bucketStart)) continue
    const existing = buckets.get(bucketStart)
    if (!existing) {
      buckets.set(bucketStart, { ...candle, timeframe, timestamp: bucketStart })
      continue
    }
    existing.high = Math.max(existing.high, candle.high)
    existing.low = Math.min(existing.low, candle.low)
    existing.close = candle.close
    existing.volume += candle.volume
  }
  return Array.from(buckets.values())
}

function mergeSortedCandles(stored: Candle[], realtimeDerived: Candle[]) {
  const merged: Candle[] = []
  let storedIndex = 0
  let realtimeIndex = 0
  while (storedIndex < stored.length || realtimeIndex < realtimeDerived.length) {
    if (storedIndex >= stored.length) {
      const realtime = lastCandleAtTimestamp(realtimeDerived, realtimeIndex)
      merged.push(realtime.candle)
      realtimeIndex = realtime.nextIndex
      continue
    }
    if (realtimeIndex >= realtimeDerived.length) {
      const storedItem = lastCandleAtTimestamp(stored, storedIndex)
      merged.push(storedItem.candle)
      storedIndex = storedItem.nextIndex
      continue
    }
    const storedCandle = stored[storedIndex]
    const realtimeCandle = realtimeDerived[realtimeIndex]
    if (storedCandle.timestamp < realtimeCandle.timestamp) {
      const storedItem = lastCandleAtTimestamp(stored, storedIndex)
      merged.push(storedItem.candle)
      storedIndex = storedItem.nextIndex
      continue
    }
    if (realtimeCandle.timestamp < storedCandle.timestamp) {
      const realtime = lastCandleAtTimestamp(realtimeDerived, realtimeIndex)
      merged.push(realtime.candle)
      realtimeIndex = realtime.nextIndex
      continue
    }
    const storedItem = lastCandleAtTimestamp(stored, storedIndex)
    const realtime = lastCandleAtTimestamp(realtimeDerived, realtimeIndex)
    merged.push(realtime.candle)
    storedIndex = storedItem.nextIndex
    realtimeIndex = realtime.nextIndex
  }
  return merged
}

function lastCandleAtTimestamp(candles: Candle[], startIndex: number) {
  const timestamp = candles[startIndex].timestamp
  let nextIndex = startIndex + 1
  while (nextIndex < candles.length && candles[nextIndex].timestamp === timestamp) {
    nextIndex += 1
  }
  return {
    candle: candles[nextIndex - 1],
    nextIndex,
  }
}
