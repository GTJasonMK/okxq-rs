import type { InstType, Timeframe } from '@/types'
import type { SyncProgressSummary } from '@/utils/syncProgress'

export type RepairProgress = SyncProgressSummary & {
  visible: boolean
}

export type MarketInstType = Extract<InstType, 'SPOT' | 'SWAP'>

export type CandleRangeDays = 1 | 3 | 7 | 14 | 30 | 90 | 180 | 365 | 730 | 1095 | 1825

export type MarketSettings = {
  activeSymbol?: string
  marketInstType?: MarketInstType
  activeTimeframe?: Timeframe
  orderbookDepth?: number
  candleRangeDays?: CandleRangeDays
}

export type PendingOrderbookRequest = {
  instId: string
  size: number
}
