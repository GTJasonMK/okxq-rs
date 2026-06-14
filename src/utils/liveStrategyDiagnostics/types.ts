import type { Candle, Timeframe } from '@/types'

export type RealtimeDecisionCandle = Candle & { confirm?: string }

export type LiveStrategyDiagnosticTarget = {
  strategy_id: string
  symbol: string
  timeframe: Timeframe
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  mode: string
  params: Record<string, unknown>
}

type LiveStrategyRuntimeForm = {
  strategy_id: string
  symbol: string
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  params: Record<string, unknown>
}

export type DiagnosticTargetInput = {
  selectedSymbol: string
  triggerTimeframe: Timeframe
  controlMode: string
  defaultInitialCapital: number
  form: LiveStrategyRuntimeForm
}
