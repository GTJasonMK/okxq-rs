import type { Candle, Timeframe } from '@/types'
import { candleRangeOptionsForTimeframe } from '@/utils/marketView'
import type { TriggerCandleRequestState } from './types'

export function triggerRangeSelectOptions(timeframe: Timeframe) {
  return candleRangeOptionsForTimeframe(timeframe).map(option => ({
    label: option.label,
    value: String(option.value),
  }))
}

export function mergeTriggerCandles(
  candles: Candle[],
  candle: Candle,
  maxRows: number,
) {
  const byTimestamp = new Map<number, Candle>()
  for (const item of candles) {
    if (Number.isFinite(item.timestamp)) byTimestamp.set(item.timestamp, item)
  }
  byTimestamp.set(candle.timestamp, candle)
  const merged = Array.from(byTimestamp.values())
    .sort((left, right) => left.timestamp - right.timestamp)
  const overflow = merged.length - maxRows
  return overflow > 0 ? merged.slice(overflow) : merged
}

export function triggerCandleRequestMatches(
  request: TriggerCandleRequestState,
  current: TriggerCandleRequestState,
) {
  return request.sequence === current.sequence &&
    request.instId === current.instId &&
    request.timeframe === current.timeframe &&
    request.rangeDays === current.rangeDays
}
