import { isValidCandle } from '@/api/marketNormalize'
import type { Candle } from '@/types'

function compareCandleTimestamp(left: Candle, right: Candle) {
  return left.timestamp - right.timestamp
}

export function sortedValidCandles(candles: Candle[]) {
  return candlesAreSortedAndValid(candles)
    ? candles
    : candles.filter(isValidCandle).sort(compareCandleTimestamp)
}

function candlesAreSortedAndValid(candles: Candle[]) {
  let previous = Number.NEGATIVE_INFINITY
  for (const candle of candles) {
    if (!isValidCandle(candle) || candle.timestamp < previous) return false
    previous = candle.timestamp
  }
  return true
}

export function lowerBoundCandleTimestamp(candles: Candle[], timestamp: number) {
  let low = 0
  let high = candles.length
  while (low < high) {
    const mid = low + Math.floor((high - low) / 2)
    if (candles[mid].timestamp < timestamp) low = mid + 1
    else high = mid
  }
  return low
}
