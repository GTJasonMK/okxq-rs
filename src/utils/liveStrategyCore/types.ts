import type { LiveEquityHistory, LiveStrategyStatus, TradingMode } from '@/types'

export type NumberRangeUnit = '' | '%' | '秒'
export type LiveEquitySnapshot = LiveEquityHistory['snapshots'][number]
export type LiveDataScope = {
  mode: TradingMode
  runId: string
}

export type DetailDataScopeTextInput = LiveDataScope & {
  status: LiveStrategyStatus | null
  hiddenOrderCount: number
  hiddenEquityByScope: boolean
  scopedEquityHistory: LiveEquityHistory | null
}
