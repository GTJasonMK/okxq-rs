import { computed, reactive, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import type { LiveStrategyStatus, StrategyMeta } from '@/types'
import {
  DEFAULT_LIVE_CONTROL_FORM,
} from '@/utils/liveStrategyControl'
import {
  applyLiveStrategyFormState,
  enforceStrategyRuntimeInstType,
  enforceStrategyRuntimeSymbol,
  enforceStrategyRuntimeTimeframe,
  liveStrategyFormFromRunningStatus,
  liveStrategyFormFromStrategy,
  strategyRuntimeSymbolOptions,
  strategyRuntimeTimeframeOptions,
  type LiveStrategyControlForm,
} from '@/utils/liveStrategyForm'
import { withCurrentOption } from '@/utils/liveStrategyCore'

type LiveStrategyConfigSelectionDeps = {
  status: Ref<LiveStrategyStatus | null>
  formLocked: ComputedRef<boolean>
}

export function useLiveStrategyConfigSelection({
  status,
  formLocked,
}: LiveStrategyConfigSelectionDeps) {
  const strategies = ref<StrategyMeta[]>([])
  const strategiesLoaded = ref(false)
  const form = reactive({ ...DEFAULT_LIVE_CONTROL_FORM })

  const selectedStrategy = computed(() =>
    strategies.value.find(strategy => strategy.id === form.strategy_id) ?? null
  )
  const strategyIds = computed(() => strategies.value.map(strategy => strategy.id))
  const strategyOptions = computed(() => withCurrentOption(
    [
      { value: '', label: '请选择策略' },
      ...strategies.value.map(strategy => ({ value: strategy.id, label: strategy.name || strategy.id })),
    ],
    formLocked.value ? form.strategy_id : '',
    form.strategy_id,
  ))
  const activeStrategyId = computed(() =>
    status.value?.running ? status.value.strategy_id : form.strategy_id
  )
  const timeframeOptions = computed(() => withCurrentOption(
    strategyTimeframeOptions(form.strategy_id),
    formLocked.value ? form.timeframe : '',
    form.timeframe,
  ))
  const symbolOptions = computed(() => withCurrentOption(
    strategySymbolOptions(form.strategy_id),
    formLocked.value ? form.symbol : '',
    form.symbol,
  ))

  function strategyTimeframeOptions(strategyId: string) {
    return strategyRuntimeTimeframeOptions(
      strategies.value,
      strategyId,
      form.timeframe || DEFAULT_LIVE_CONTROL_FORM.timeframe,
    )
  }

  function strategySymbolOptions(strategyId: string) {
    return strategyRuntimeSymbolOptions(
      strategies.value,
      strategyId,
      form.symbol || DEFAULT_LIVE_CONTROL_FORM.symbol,
    )
  }

  function setStrategies(nextStrategies: StrategyMeta[]) {
    strategies.value = nextStrategies
    strategiesLoaded.value = true
  }

  function applyStrategyRuntime(strategyId: string, force = false) {
    if (formLocked.value && !force) return false
    const strategy = strategies.value.find(item => item.id === strategyId)
    if (!strategy) return false
    applyLiveStrategyFormState(form, liveStrategyFormFromStrategy(strategy, DEFAULT_LIVE_CONTROL_FORM))
    return true
  }

  function setStrategyId(value: string) {
    if (formLocked.value) return false
    const strategyId = value.trim()
    if (!strategyId) {
      form.strategy_id = ''
      form.params = {}
      return true
    }
    if (!applyStrategyRuntime(strategyId)) {
      form.strategy_id = strategyId
      form.params = {}
    }
    return true
  }

  function reconcileStrategyAvailability() {
    if (!strategiesLoaded.value) return false
    if (form.strategy_id && strategyIds.value.includes(form.strategy_id)) {
      if (!form.symbol || !form.timeframe) return applyStrategyRuntime(form.strategy_id)
      return false
    }
    const firstStrategy = strategies.value[0]
    if (firstStrategy) return applyStrategyRuntime(firstStrategy.id)
    form.strategy_id = ''
    return true
  }

  function syncFormWithRunningStatus(current: LiveStrategyStatus) {
    const nextForm = liveStrategyFormFromRunningStatus(current, form)
    if (!nextForm) return false
    applyLiveStrategyFormState(form, nextForm)
    return true
  }

  function enforceSupportedTimeframe() {
    enforceStrategyRuntimeTimeframe(form, selectedStrategy.value)
  }

  function enforceSupportedSymbol() {
    enforceStrategyRuntimeSymbol(form, selectedStrategy.value)
    enforceStrategyRuntimeInstType(form, selectedStrategy.value)
  }

  return {
    activeStrategyId,
    form: form as LiveStrategyControlForm,
    selectedStrategy,
    strategies,
    strategiesLoaded,
    strategyIds,
    strategyOptions,
    timeframeOptions,
    symbolOptions,
    strategyTimeframeOptions,
    strategySymbolOptions,
    setStrategies,
    applyStrategyRuntime,
    setStrategyId,
    reconcileStrategyAvailability,
    syncFormWithRunningStatus,
    enforceSupportedTimeframe,
    enforceSupportedSymbol,
  }
}
