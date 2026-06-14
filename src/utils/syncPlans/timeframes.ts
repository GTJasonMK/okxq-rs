import type { Timeframe } from '@/types'
import { SYNC_TIMEFRAMES } from './constants'

export function isSupportedTimeframe(value: string): value is Timeframe {
  return SYNC_TIMEFRAMES.includes(value as Timeframe)
}

export function timeframeOrder(value: string): number {
  const index = SYNC_TIMEFRAMES.indexOf(value as Timeframe)
  return index >= 0 ? index : 999
}
