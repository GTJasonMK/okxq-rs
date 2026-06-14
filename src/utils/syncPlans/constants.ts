import type { Timeframe } from '@/types'

export const BASE_SYNC_TIMEFRAME: Timeframe = '1m'

export const SYNC_TIMEFRAMES: Timeframe[] = [
  BASE_SYNC_TIMEFRAME,
  '3m',
  '5m',
  '15m',
  '30m',
  '1H',
  '2H',
  '4H',
  '6H',
  '12H',
  '1D',
  '1W',
  '1M',
]

export const DEFAULT_SYNC_DAYS: Record<Timeframe, number> = {
  '1m': 90,
  '3m': 90,
  '5m': 90,
  '15m': 90,
  '30m': 90,
  '1H': 120,
  '2H': 180,
  '4H': 365,
  '6H': 365,
  '12H': 730,
  '1D': 3650,
  '1W': 3650,
  '1M': 3650,
}

export const DEFAULT_UNIFIED_SYNC_DAYS = 90

export const COMMON_SYNC_TIMEFRAMES = new Set<Timeframe>(['1m', '5m', '15m', '1H', '4H', '1D'])
