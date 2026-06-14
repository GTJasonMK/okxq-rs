import type { Timeframe } from '@/types'
import {
  cloneParams,
  stableJson,
} from '@/utils/liveStrategyCore'
import type {
  DiagnosticTargetInput,
  LiveStrategyDiagnosticTarget,
} from '@/utils/liveStrategyDiagnostics/types'

export function buildDiagnosticTarget(input: DiagnosticTargetInput): LiveStrategyDiagnosticTarget {
  const selectedSymbol = input.selectedSymbol || input.form.symbol
  return {
    strategy_id: input.form.strategy_id,
    symbol: selectedSymbol,
    timeframe: input.triggerTimeframe as Timeframe,
    initial_capital: Number.isFinite(input.form.initial_capital)
      ? input.form.initial_capital
      : input.defaultInitialCapital,
    position_size: input.form.position_size,
    stop_loss: input.form.stop_loss,
    take_profit: input.form.take_profit,
    mode: input.controlMode,
    params: cloneParams(input.form.params),
  }
}

export function diagnosticTargetKey(target: LiveStrategyDiagnosticTarget) {
  return stableJson({
    strategy_id: target.strategy_id,
    symbol: target.symbol,
    timeframe: target.timeframe,
    initial_capital: target.initial_capital,
    position_size: target.position_size,
    stop_loss: target.stop_loss,
    take_profit: target.take_profit,
    mode: target.mode,
    params: target.params,
  })
}
