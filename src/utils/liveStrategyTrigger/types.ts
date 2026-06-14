import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'

export type LiveStrategySelectOption = { value: string; label: string }
export type RealtimeTriggerCandle = Candle & { confirm?: string }

export type TriggerCandleRequestState = {
  sequence: number
  instId: string
  timeframe: Timeframe
  rangeDays: CandleRangeDays
}
