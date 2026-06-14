import type { Timeframe, WatchedSymbolSyncPlan } from '@/types'
import { COMMON_SYNC_TIMEFRAMES, DEFAULT_UNIFIED_SYNC_DAYS } from './constants'

export function defaultSyncPlan(timeframe: Timeframe): WatchedSymbolSyncPlan {
  return {
    timeframe,
    enabled: COMMON_SYNC_TIMEFRAMES.has(timeframe),
    bootstrap_days: DEFAULT_UNIFIED_SYNC_DAYS,
    archive_mode: 'rolling',
  }
}

export function disabledSyncPlan(timeframe: Timeframe): WatchedSymbolSyncPlan {
  return {
    ...defaultSyncPlan(timeframe),
    enabled: false,
  }
}
