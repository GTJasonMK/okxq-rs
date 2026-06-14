import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import {
  CANDLE_RANGE_OPTIONS,
  DAY_MS,
  DEFAULT_CANDLE_RANGE_DAYS,
  MAX_CHART_CANDLE_ROWS,
} from '@/utils/marketView/constants'
import { normalizeCandleRangeDays } from '@/utils/marketView/settings'
import { timeframeToMs } from './time'
import { lowerBoundCandleTimestamp, sortedValidCandles } from './sorted'

export function candleLimitForRange(timeframe: Timeframe, rangeDays: CandleRangeDays) {
  return Math.min(MAX_CHART_CANDLE_ROWS, estimatedCandleCountForRange(timeframe, rangeDays))
}

function estimatedCandleCountForRange(timeframe: Timeframe, rangeDays: CandleRangeDays) {
  const timeframeMs = Math.max(timeframeToMs(timeframe), 1)
  return Math.max(1, Math.ceil(rangeDays * DAY_MS / timeframeMs))
}

export function candleRangeOptionsForTimeframe(timeframe: Timeframe) {
  const options = CANDLE_RANGE_OPTIONS.filter(option =>
    estimatedCandleCountForRange(timeframe, option.value) <= MAX_CHART_CANDLE_ROWS
  )
  return options.length > 0 ? options : CANDLE_RANGE_OPTIONS.slice(0, 1)
}

export function clampCandleRangeDaysForTimeframe(
  value: unknown,
  timeframe: Timeframe,
): CandleRangeDays {
  const options = candleRangeOptionsForTimeframe(timeframe).map(option => option.value)
  const normalized = normalizeCandleRangeDays(value) ?? DEFAULT_CANDLE_RANGE_DAYS
  return [...options].reverse().find(option => option <= normalized) ?? options[0] ?? DEFAULT_CANDLE_RANGE_DAYS
}

export function filterCandlesByRange(
  candles: Candle[],
  timeframe: Timeframe,
  rangeDays: CandleRangeDays,
) {
  if (candles.length === 0) return []
  const sorted = sortedValidCandles(candles)
  if (sorted.length === 0) return []
  return filterSortedCandlesByRange(sorted, timeframe, rangeDays)
}

export function filterSortedCandlesByRange(
  candles: Candle[],
  timeframe: Timeframe,
  rangeDays: CandleRangeDays,
) {
  if (candles.length === 0) return []
  const latest = candles[candles.length - 1]?.timestamp ?? 0
  if (!Number.isFinite(latest) || latest <= 0) return candles.slice(-MAX_CHART_CANDLE_ROWS)
  const cutoff = latest - rangeDays * DAY_MS + timeframeToMs(timeframe)
  const startIndex = Math.max(
    lowerBoundCandleTimestamp(candles, cutoff),
    candles.length - MAX_CHART_CANDLE_ROWS,
  )
  return candles.slice(startIndex)
}
