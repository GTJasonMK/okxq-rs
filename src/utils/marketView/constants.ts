import type { Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'

export const DEFAULT_DEPTH_ORDERBOOK_SIZE = 400
export const MAX_DEPTH_ORDERBOOK_SIZE = 5000
export const DEFAULT_CANDLE_RANGE_DAYS: CandleRangeDays = 7
export const MAX_CHART_CANDLE_ROWS = 100_000
export const LATEST_CANDLE_VIEW_RATIO = 0.5
export const DAY_MS = 86_400_000

export const CANDLE_RANGE_OPTIONS: Array<{ value: CandleRangeDays; label: string }> = [
  { value: 1, label: '1天' },
  { value: 3, label: '3天' },
  { value: 7, label: '7天' },
  { value: 14, label: '14天' },
  { value: 30, label: '30天' },
  { value: 90, label: '90天' },
  { value: 180, label: '180天' },
  { value: 365, label: '1年' },
  { value: 730, label: '2年' },
  { value: 1095, label: '3年' },
  { value: 1825, label: '5年' },
]

export const VALID_CANDLE_RANGE_DAYS = CANDLE_RANGE_OPTIONS.map(item => item.value)

export const VALID_MARKET_TIMEFRAMES: Timeframe[] = [
  '1m',
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
