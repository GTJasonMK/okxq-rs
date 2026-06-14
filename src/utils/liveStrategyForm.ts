import { isRecord } from '@/api/normalize'
import type { InstType, LiveStrategyStatus, StrategyMeta, Timeframe } from '@/types'
import {
  DEFAULT_LIVE_CONTROL_FORM,
} from '@/utils/liveStrategyControl'
import {
  cloneParams,
  numberField,
  positiveNumberField,
} from '@/utils/liveStrategyCore'

export type LiveStrategySelectOption = { value: string; label: string }
export type LiveStrategyControlForm = {
  strategy_id: string
  symbol: string
  inst_type: InstType
  timeframe: Timeframe
  risk_timeframe: Timeframe
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  check_interval: number
  params: Record<string, unknown>
}

export function liveStrategyFormFromStrategy(
  strategy: StrategyMeta,
  defaults: LiveStrategyControlForm = DEFAULT_LIVE_CONTROL_FORM,
): LiveStrategyControlForm {
  const runtime = strategy.runtime
  return {
    strategy_id: strategy.id,
    symbol: runtime?.symbol || defaults.symbol,
    inst_type: runtime?.inst_type || defaults.inst_type,
    timeframe: runtime?.timeframe || defaults.timeframe,
    risk_timeframe: runtime?.risk_timeframe || defaults.risk_timeframe,
    initial_capital: positiveNumberField(runtime?.initial_capital, defaults.initial_capital),
    position_size: numberField(runtime?.position_size, defaults.position_size),
    stop_loss: numberField(runtime?.stop_loss, defaults.stop_loss),
    take_profit: numberField(runtime?.take_profit, defaults.take_profit),
    check_interval: positiveNumberField(runtime?.check_interval, defaults.check_interval),
    params: isRecord(runtime?.params) ? cloneParams(runtime.params) : {},
  }
}

export function liveStrategyFormFromRunningStatus(
  status: LiveStrategyStatus,
  current: LiveStrategyControlForm,
): LiveStrategyControlForm | null {
  if (!status.running) return null
  return {
    strategy_id: status.strategy_id,
    symbol: status.symbol || current.symbol,
    inst_type: status.inst_type || current.inst_type,
    timeframe: status.timeframe || current.timeframe,
    risk_timeframe: status.risk_timeframe || current.risk_timeframe,
    initial_capital: positiveNumberField(status.initial_capital, current.initial_capital),
    position_size: positiveNumberField(status.position_size, current.position_size),
    stop_loss: numberField(status.stop_loss, current.stop_loss),
    take_profit: numberField(status.take_profit, current.take_profit),
    check_interval: positiveNumberField(status.check_interval, current.check_interval),
    params: isRecord(status.params) ? cloneParams(status.params) : {},
  }
}

export function applyLiveStrategyFormState(
  form: LiveStrategyControlForm,
  state: LiveStrategyControlForm,
) {
  form.strategy_id = state.strategy_id
  form.symbol = state.symbol
  form.inst_type = state.inst_type
  form.timeframe = state.timeframe
  form.risk_timeframe = state.risk_timeframe
  form.initial_capital = state.initial_capital
  form.position_size = state.position_size
  form.stop_loss = state.stop_loss
  form.take_profit = state.take_profit
  form.check_interval = state.check_interval
  form.params = cloneParams(state.params)
}

export function strategyRuntimeTimeframeOptions(
  strategies: StrategyMeta[],
  strategyId: string,
  fallbackTimeframe: Timeframe,
): LiveStrategySelectOption[] {
  const timeframe = strategies.find(item => item.id === strategyId)?.runtime?.timeframe || fallbackTimeframe
  return timeframe ? [{ value: timeframe, label: timeframe }] : []
}

export function strategyRuntimeSymbolOptions(
  strategies: StrategyMeta[],
  strategyId: string,
  fallbackSymbol: string,
): LiveStrategySelectOption[] {
  const symbol = strategies.find(item => item.id === strategyId)?.runtime?.symbol || fallbackSymbol
  return symbol ? [{ value: symbol, label: symbol }] : []
}

export function enforceStrategyRuntimeTimeframe(
  form: LiveStrategyControlForm,
  strategy: StrategyMeta | null,
) {
  if (strategy?.runtime?.timeframe) form.timeframe = strategy.runtime.timeframe
}

export function enforceStrategyRuntimeSymbol(
  form: LiveStrategyControlForm,
  strategy: StrategyMeta | null,
) {
  if (strategy?.runtime?.symbol) form.symbol = strategy.runtime.symbol
}

export function enforceStrategyRuntimeInstType(
  form: LiveStrategyControlForm,
  strategy: StrategyMeta | null,
) {
  if (strategy?.runtime?.inst_type) form.inst_type = strategy.runtime.inst_type
}
