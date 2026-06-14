import type { Candle } from '@/types/market'
import {
  numberValue,
  stringValue as textValue,
  timestampNumber as timestampValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeInstId,
  normalizeInstType,
  normalizeTimeframe,
} from './core'

export function normalizeCandle(raw: Record<string, unknown>): Candle {
  const rawInstId = textValue(raw.inst_id)
  const instType = normalizeInstType(raw.inst_type, inferInstTypeFromId(rawInstId))
  const timeframe = normalizeTimeframe(textValue(raw.timeframe, '1H')) || '1H'
  const candle: Candle = {
    inst_id: normalizeInstId(rawInstId, instType),
    inst_type: instType,
    timeframe: timeframe as Candle['timeframe'],
    timestamp: timestampValue(raw.timestamp),
    open: numberValue(raw.open),
    high: numberValue(raw.high),
    low: numberValue(raw.low),
    close: numberValue(raw.close),
    volume: numberValue(raw.volume),
  }
  const volumeCcy = nonNegativeNumber(raw.volume_ccy)
  const volumeQuote = nonNegativeNumber(raw.volume_quote)
  if (volumeCcy !== null) candle.volume_ccy = volumeCcy
  if (volumeQuote !== null) candle.volume_quote = volumeQuote
  return candle
}

export function isValidCandle(candle: Candle): boolean {
  return Boolean(candle.inst_id && candle.timeframe)
    && Number.isFinite(candle.timestamp)
    && candle.timestamp > 0
    && Number.isFinite(candle.open)
    && candle.open > 0
    && Number.isFinite(candle.high)
    && candle.high > 0
    && Number.isFinite(candle.low)
    && candle.low > 0
    && Number.isFinite(candle.close)
    && candle.close > 0
    && Number.isFinite(candle.volume)
    && candle.volume >= 0
    && optionalNonNegativeNumber(candle.volume_ccy)
    && optionalNonNegativeNumber(candle.volume_quote)
}

function nonNegativeNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function optionalNonNegativeNumber(value: unknown): boolean {
  return value === undefined || nonNegativeNumber(value) !== null
}
