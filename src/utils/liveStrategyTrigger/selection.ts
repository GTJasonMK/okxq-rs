import type { LiveOrder, Timeframe } from '@/types'
import { addSymbolOption, withCurrentOption } from '@/utils/liveStrategyCore'
import type { LiveStrategySelectOption } from './types'

export function triggerTimeframeOptionsForContext(
  strategyTimeframeOptions: LiveStrategySelectOption[],
  runningTimeframe: Timeframe | '',
) {
  return withCurrentOption(
    strategyTimeframeOptions,
    runningTimeframe,
    runningTimeframe,
  )
}

export function resolveTriggerTimeframe(
  selectedTimeframe: Timeframe | '',
  options: LiveStrategySelectOption[],
  preferredTimeframe: Timeframe,
) {
  const values = options.map(option => option.value as Timeframe)
  if (values.includes(selectedTimeframe as Timeframe)) return selectedTimeframe as Timeframe
  return values.includes(preferredTimeframe) ? preferredTimeframe : values[0] ?? preferredTimeframe
}

export function triggerSymbolOptionsForContext(
  symbolOptions: LiveStrategySelectOption[],
  statusSymbol: string | undefined,
  formSymbol: string,
  orders: LiveOrder[],
) {
  const values = new Set<string>()
  for (const option of symbolOptions) addSymbolOption(values, option.value)
  addSymbolOption(values, statusSymbol)
  addSymbolOption(values, formSymbol)
  for (const order of orders) addSymbolOption(values, order.inst_id)
  return Array.from(values).map(value => ({ value, label: value }))
}

export function selectedTriggerOptionValue(
  options: LiveStrategySelectOption[],
  currentValue: string,
) {
  if (options.length === 0) return ''
  const values = options.map(option => option.value)
  return values.includes(currentValue) ? currentValue : values[0]
}
