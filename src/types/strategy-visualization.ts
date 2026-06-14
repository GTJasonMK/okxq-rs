import type { Timestamp } from './common'

export type StrategyTriggerKind = 'entry' | 'exit' | 'blocked' | 'risk' | 'pending'
type StrategyTriggerSource = 'backtest' | 'live' | 'simulated'
export type StrategyTriggerMarkerMode = 'auto' | 'text' | 'icon' | 'hidden'

export interface StrategyTriggerMarker {
  id: string
  timestamp: Timestamp
  price: number
  side: string
  kind: StrategyTriggerKind
  source: StrategyTriggerSource
  label: string
  instId?: string
  status?: string
  reason?: string
  detail?: string
  pnl?: number
  pnlPct?: number
}
