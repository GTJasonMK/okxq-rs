import { normalizeBaseSymbol } from '@/api/market'
import { isRecord, numberValue, stringValue } from '@/api/normalize'
import type { Timeframe } from '@/types'
import type { CandleRangeDays, MarketInstType, MarketSettings } from '@/types/marketView'
import {
  DEFAULT_DEPTH_ORDERBOOK_SIZE,
  MAX_DEPTH_ORDERBOOK_SIZE,
  VALID_CANDLE_RANGE_DAYS,
  VALID_MARKET_TIMEFRAMES,
} from '@/utils/marketView/constants'

export function normalizeMarketSettings(value: unknown): MarketSettings {
  if (!isRecord(value)) return {}
  const activeSymbol = normalizeBaseSymbol(stringValue(value.activeSymbol))
  const marketInstType = normalizeMarketType(value.marketInstType)
  const activeTimeframe = normalizeTimeframe(value.activeTimeframe)
  const orderbookDepth = clampOrderbookSize(numberValue(value.orderbookDepth, DEFAULT_DEPTH_ORDERBOOK_SIZE))
  const candleRangeDays = normalizeCandleRangeDays(value.candleRangeDays)
  return {
    activeSymbol: activeSymbol || undefined,
    marketInstType: marketInstType || undefined,
    activeTimeframe: activeTimeframe || undefined,
    orderbookDepth,
    candleRangeDays,
  }
}

export function marketSettingsPayload(settings: MarketSettings): MarketSettings {
  return {
    ...settings,
    marketInstType: settings.marketInstType,
  }
}

export function normalizeMarketType(value: unknown): MarketInstType | '' {
  const raw = stringValue(value).trim().toUpperCase()
  if (raw === 'SPOT' || raw === 'SWAP') return raw
  return ''
}

export function normalizeTimeframe(value: unknown): Timeframe | '' {
  const raw = stringValue(value).trim()
  if (VALID_MARKET_TIMEFRAMES.includes(raw as Timeframe)) return raw as Timeframe
  const aliasMap: Record<string, Timeframe> = {
    '1h': '1H',
    '2h': '2H',
    '4h': '4H',
    '6h': '6H',
    '12h': '12H',
    '1d': '1D',
    '1w': '1W',
  }
  return aliasMap[raw] ?? ''
}

export function normalizeCandleRangeDays(value: unknown): CandleRangeDays | undefined {
  const raw = stringValue(value).trim().toLowerCase()
  const yearMatch = raw.match(/^(\d+(?:\.\d+)?)\s*(?:y|yr|yrs|year|years|年)$/)
  if (yearMatch) {
    const roundedYearDays = Math.round(Number(yearMatch[1]) * 365) as CandleRangeDays
    return VALID_CANDLE_RANGE_DAYS.includes(roundedYearDays) ? roundedYearDays : undefined
  }
  const normalizedText = raw.replace(/days?|d|天/g, '')
  const numeric = numberValue(normalizedText ? Number(normalizedText) : value, Number.NaN)
  if (!Number.isFinite(numeric)) return undefined
  const rounded = Math.round(numeric) as CandleRangeDays
  return VALID_CANDLE_RANGE_DAYS.includes(rounded) ? rounded : undefined
}

export function clampOrderbookSize(size: number) {
  const candidate = Number.isFinite(size) ? size : DEFAULT_DEPTH_ORDERBOOK_SIZE
  return Math.max(1, Math.min(MAX_DEPTH_ORDERBOOK_SIZE, Math.round(candidate)))
}
