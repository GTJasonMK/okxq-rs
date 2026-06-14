import { computed, ref, watch } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import type { LiveOrder, LiveStrategyStatus, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import {
  clampCandleRangeDaysForTimeframe,
  DEFAULT_CANDLE_RANGE_DAYS,
} from '@/utils/marketView'
import {
  resolveTriggerTimeframe,
  selectedTriggerOptionValue,
  triggerRangeSelectOptions,
  triggerSymbolOptionsForContext,
  triggerTimeframeOptionsForContext,
  type LiveStrategySelectOption,
} from '@/utils/liveStrategyTrigger'

type LiveTriggerSelectionForm = {
  symbol: string
  timeframe: Timeframe
}

type LiveTriggerSelectionDeps = {
  status: Ref<LiveStrategyStatus | null>
  form: LiveTriggerSelectionForm
  activeStrategyId: ComputedRef<string>
  symbolOptions: ComputedRef<LiveStrategySelectOption[]>
  orders: Ref<LiveOrder[]>
  strategyTimeframeOptions: (strategyId: string) => LiveStrategySelectOption[]
}

export function useLiveStrategyTriggerSelection({
  status,
  form,
  activeStrategyId,
  symbolOptions,
  orders,
  strategyTimeframeOptions,
}: LiveTriggerSelectionDeps) {
  const selectedTriggerSymbol = ref('')
  const selectedTriggerTimeframe = ref<Timeframe | ''>('')
  const triggerRangeDays = ref<CandleRangeDays>(DEFAULT_CANDLE_RANGE_DAYS)

  const triggerTimeframeOptions = computed(() => triggerTimeframeOptionsForContext(
    strategyTimeframeOptions(activeStrategyId.value),
    status.value?.running ? status.value.timeframe : '',
  ))
  const triggerTimeframe = computed(() => resolveTriggerTimeframe(
    selectedTriggerTimeframe.value,
    triggerTimeframeOptions.value,
    status.value?.running ? status.value.timeframe : form.timeframe,
  ))
  const triggerSymbolOptions = computed(() => triggerSymbolOptionsForContext(
    symbolOptions.value,
    status.value?.symbol,
    form.symbol,
    orders.value,
  ))
  const triggerRangeOptions = computed(() => triggerRangeSelectOptions(triggerTimeframe.value))

  function setTriggerSymbol(value: string) {
    selectedTriggerSymbol.value = selectedTriggerOptionValue(triggerSymbolOptions.value, value)
  }

  function setTriggerTimeframe(value: string) {
    selectedTriggerTimeframe.value = selectedTriggerOptionValue(
      triggerTimeframeOptions.value,
      value,
    ) as Timeframe | ''
    syncSelectedTriggerSymbol()
    syncTriggerRangeDays()
  }

  function setTriggerRangeDays(value: string) {
    const parsed = Number(value) as CandleRangeDays
    if (triggerRangeOptions.value.some(option => option.value === value)) {
      triggerRangeDays.value = parsed
    }
  }

  function syncSelectedTriggerSymbol() {
    const selected = selectedTriggerOptionValue(triggerSymbolOptions.value, selectedTriggerSymbol.value)
    if (!selected) {
      selectedTriggerSymbol.value = ''
      return
    }
    selectedTriggerSymbol.value = selected
  }

  function syncSelectedTriggerTimeframe() {
    if (triggerTimeframeOptions.value.length === 0) {
      selectedTriggerTimeframe.value = ''
      return
    }
    selectedTriggerTimeframe.value = resolveTriggerTimeframe(
      selectedTriggerTimeframe.value,
      triggerTimeframeOptions.value,
      status.value?.running ? status.value.timeframe : form.timeframe,
    )
    syncTriggerRangeDays()
  }

  function syncTriggerRangeDays() {
    const clamped = clampCandleRangeDaysForTimeframe(triggerRangeDays.value, triggerTimeframe.value)
    if (clamped !== triggerRangeDays.value) {
      triggerRangeDays.value = clamped
    }
  }

  function syncTriggerSelectionForRunningStatus(current: LiveStrategyStatus) {
    selectedTriggerTimeframe.value = current.timeframe || form.timeframe
    selectedTriggerSymbol.value = current.symbol || form.symbol
  }

  watch(triggerSymbolOptions, () => {
    syncSelectedTriggerSymbol()
  })

  watch(triggerTimeframeOptions, () => {
    syncSelectedTriggerTimeframe()
    syncSelectedTriggerSymbol()
  })

  return {
    selectedTriggerSymbol,
    selectedTriggerTimeframe,
    triggerRangeDays,
    triggerTimeframe,
    triggerTimeframeOptions,
    triggerSymbolOptions,
    triggerRangeOptions,
    setTriggerSymbol,
    setTriggerTimeframe,
    setTriggerRangeDays,
    syncSelectedTriggerSymbol,
    syncSelectedTriggerTimeframe,
    syncTriggerSelectionForRunningStatus,
  }
}
