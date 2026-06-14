import { computed, onMounted, ref, watch } from 'vue'
import { useSystemStore } from '@/stores/systemStore'
import type {
  LiveExecutionPlan,
  LiveOrder,
  LiveEquityHistory,
  LiveStrategyStatus,
  Position,
  Timeframe,
} from '@/types'
import { describeError } from '@/utils/logger'
import {
  DEFAULT_LIVE_CONTROL_FORM as DEFAULT_FORM,
} from '@/utils/liveStrategyControl'
import {
  modeLabel,
} from '@/utils/liveStrategyCore'
import {
  liveActionBusyText,
  liveLaunchReadiness,
  liveRiskScopeNote,
  type LiveActionPhase,
} from '@/utils/liveStrategyControlView'
import {
  liveTriggerSubtitleText,
} from '@/utils/liveStrategyTrigger'
import { liveRuntimeConfigDisabledReason } from '@/utils/liveStrategyRuntimeConfig'
import { liveOrdersToMarkers } from '@/utils/strategyTriggers'
import { useLiveStrategyActions } from './useLiveStrategyActions'
import { useLiveStrategyConfigSelection } from './useLiveStrategyConfigSelection'
import { useLiveStrategyDiagnostics } from './useLiveStrategyDiagnostics'
import { useLiveStrategyRuntimeData } from './useLiveStrategyRuntimeData'
import { useLiveStrategyTriggerSelection } from './useLiveStrategyTriggerSelection'
import { useLiveTriggerCandles } from './useLiveTriggerCandles'

export function useLiveStrategyView() {
  const systemStore = useSystemStore()
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)
  const actionLoading = ref(false)
  const actionPhase = ref<LiveActionPhase>('idle')
  const status = ref<LiveStrategyStatus | null>(null)
  const executionPlans = ref<LiveExecutionPlan[]>([])
  const orders = ref<LiveOrder[]>([])
  const positions = ref<Position[]>([])
  const equityHistory = ref<LiveEquityHistory | null>(null)
  const runParamModalOpen = ref(false)

  const formLocked = computed(() => Boolean(status.value?.running))
  const launchMode = computed(() => systemStore.tradingMode)
  const launchModeLabel = computed(() => systemStore.tradingModeLabel)
  const controlMode = computed(() => status.value?.running ? status.value.mode : launchMode.value)
  const controlModeLabel = computed(() =>
    status.value?.running ? `${modeLabel(status.value.mode)}（当前运行）` : launchModeLabel.value
  )
  const {
    activeStrategyId,
    form,
    selectedStrategy,
    strategies,
    strategiesLoaded,
    strategyIds,
    strategyOptions,
    timeframeOptions,
    symbolOptions,
    strategyTimeframeOptions,
    setStrategies,
    applyStrategyRuntime: applyConfigStrategyRuntime,
    setStrategyId: setConfigStrategyId,
    reconcileStrategyAvailability: reconcileConfigStrategyAvailability,
    syncFormWithRunningStatus: syncConfigFormWithRunningStatus,
    enforceSupportedTimeframe,
    enforceSupportedSymbol,
  } = useLiveStrategyConfigSelection({
    status,
    formLocked,
  })
  const actionBusyText = computed(() => liveActionBusyText(actionPhase.value))
  const startDisabledReason = computed(() => {
    if (actionLoading.value) return actionBusyText.value
    if (formLocked.value) return '策略运行中，请先停止后再启动新配置'
    if (!form.strategy_id) return '请先选择策略'
    if (strategiesLoaded.value && !strategyIds.value.includes(form.strategy_id)) {
      return '当前策略未被后端发现，请重新选择'
    }
    const configError = runtimeConfigDisabledReason()
    if (configError) return configError
    return ''
  })
  const stopDisabledReason = computed(() => {
    if (actionLoading.value) return actionBusyText.value
    if (!status.value?.running) return '当前没有运行中的策略'
    return ''
  })
  const startButtonText = computed(() => actionPhase.value === 'starting' ? '启动中...' : '选择参数并启动')
  const stopButtonText = computed(() => actionPhase.value === 'stopping' ? '停止中...' : '停止')
  const launchReadiness = computed(() => liveLaunchReadiness({
    actionLoading: actionLoading.value,
    actionPhase: actionPhase.value,
    actionBusyText: actionBusyText.value,
    formLocked: formLocked.value,
    startDisabledReason: startDisabledReason.value,
    launchMode: launchMode.value,
    form,
  }))
  const {
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
  } = useLiveStrategyTriggerSelection({
    status,
    form,
    activeStrategyId,
    symbolOptions,
    orders,
    strategyTimeframeOptions,
  })
  const {
    triggerCandles,
    latestRealtimeTriggerCandle,
	  } = useLiveTriggerCandles({
	    selectedSymbol: selectedTriggerSymbol,
	    timeframe: triggerTimeframe,
	    rangeDays: triggerRangeDays,
	    onRealtimeCandle: scheduleDecisionDiagnosticsRefresh,
	    onRealtimeError: (e) => {
	      error.value = `实时决策 K 线订阅: ${describeError(e)}`
	    },
	  })
	  const {
	    decisionDiagnostics,
	    currentDecisionDiagnostics,
	    decisionDiagnosticsScopeText,
	    autoDecisionDiagnosticsEnabled,
	    decisionDiagnosticsLoading,
	    decisionDiagnosticsRefreshSource,
	    decisionDiagnosticsError,
	    scheduleDecisionDiagnosticsRefresh: scheduleDecisionDiagnosticsForRealtimeCandle,
	    clearAutoDecisionDiagnostics,
	    syncAutoDecisionDiagnosticsAfterDataLoad,
	    loadDecisionDiagnostics,
	  } = useLiveStrategyDiagnostics({
    status,
    form,
    controlMode,
    selectedTriggerSymbol,
    triggerTimeframe,
	    latestRealtimeTriggerCandle,
    defaultInitialCapital: DEFAULT_FORM.initial_capital,
    error,
  })
  const {
    activeDataMode,
    detailDataScope,
    executionLogRefreshError,
    executionLogs,
    scopedExecutionPlans,
    loadData,
    runtimeRefreshNotice,
    scopedEquityHistory,
    scopedOrders,
  } = useLiveStrategyRuntimeData({
    status,
	    executionPlans,
	    orders,
	    positions,
    equityHistory,
    launchMode,
    form,
    strategies,
    error,
    setStrategies,
    reconcileStrategyAvailability,
	    syncFormWithRunningStatus,
	    applyStrategyRuntime,
	    syncSelectedTriggerSymbol,
	    clearAutoDecisionDiagnostics,
	    syncAutoDecisionDiagnosticsAfterDataLoad,
	  })
  const riskScopeNote = computed(() => liveRiskScopeNote())
  const liveTriggerMarkers = computed(() => {
    const symbol = selectedTriggerSymbol.value
    const timeframe = triggerTimeframe.value
    return liveOrdersToMarkers(triggerScopedOrders(symbol, timeframe))
  })
  const currentTriggerOrders = computed(() => {
    const symbol = selectedTriggerSymbol.value
    const timeframe = triggerTimeframe.value
    return triggerScopedOrders(symbol, timeframe)
  })
  const liveTriggerSubtitle = computed(() => {
    return liveTriggerSubtitleText({
      symbol: selectedTriggerSymbol.value,
      timeframe: triggerTimeframe.value,
      latestCandle: triggerCandles.value.at(-1) ?? null,
      latestRealtimeCandle: latestRealtimeTriggerCandle.value,
      markerCount: liveTriggerMarkers.value.length,
    })
  })
  function applyStrategyRuntime(strategyId: string, force = false) {
    const applied = applyConfigStrategyRuntime(strategyId, force)
    if (applied) syncTriggerSelection()
    return applied
  }

  function setStrategyId(value: string) {
    if (setConfigStrategyId(value)) {
      syncSelectedTriggerTimeframe()
      syncSelectedTriggerSymbol()
    }
  }

  function setInitialCapital(value: number) {
    if (!Number.isFinite(value) || value <= 0) return
    form.initial_capital = value
  }

  function reconcileStrategyAvailability() {
    if (reconcileConfigStrategyAvailability()) syncTriggerSelection()
  }

  function syncFormWithRunningStatus(current: LiveStrategyStatus) {
    const synced = syncConfigFormWithRunningStatus(current)
    if (synced) syncTriggerSelectionForRunningStatus(current)
    return synced
  }

	  function scheduleDecisionDiagnosticsRefresh(confirmed = false) {
	    scheduleDecisionDiagnosticsForRealtimeCandle(confirmed)
	  }

  function syncTriggerSelection() {
    syncSelectedTriggerTimeframe()
    syncSelectedTriggerSymbol()
  }

  function triggerScopedOrders(
    symbol: string,
    timeframe: Timeframe,
  ) {
    void timeframe
    return scopedOrders.value
      .filter(order => !symbol || order.inst_id === symbol || order.symbol === symbol)
  }

  function runtimeConfigDisabledReason() {
    return liveRuntimeConfigDisabledReason({
      form,
      supportedTimeframes: timeframeOptions.value.map(option => option.value),
      supportedSymbols: symbolOptions.value.map(option => option.value),
    })
  }

  const { startStrategy, stopStrategy } = useLiveStrategyActions({
    systemStore,
    status,
    error,
    message,
    actionLoading,
    actionPhase,
    form,
    launchMode,
    startDisabledReason,
    stopDisabledReason,
    enforceSupportedTimeframe,
    enforceSupportedSymbol,
    loadData,
  })

  function openRunParamModal() {
    if (startDisabledReason.value) {
      error.value = startDisabledReason.value
      return
    }
    runParamModalOpen.value = true
  }

  async function submitRunParams(payload: Record<string, unknown>) {
    runParamModalOpen.value = false
    applyRunParamPayload(payload)
    await startStrategy(payload)
  }

  function applyRunParamPayload(payload: Record<string, unknown>) {
    if (typeof payload.initial_capital === 'number') form.initial_capital = payload.initial_capital
    if (typeof payload.position_size === 'number') form.position_size = payload.position_size
    if (typeof payload.stop_loss === 'number') form.stop_loss = payload.stop_loss
    if (typeof payload.take_profit === 'number') form.take_profit = payload.take_profit
    if (typeof payload.check_interval === 'number') form.check_interval = payload.check_interval
    if (typeof payload.risk_timeframe === 'string' && payload.risk_timeframe) {
      form.risk_timeframe = payload.risk_timeframe as Timeframe
    }
    if (isRecord(payload.params)) form.params = payload.params
  }

  onMounted(() => {
    const modeBeforeConfig = launchMode.value
    void loadData()
    if (!systemStore.statusLoaded) {
      void systemStore.loadConfig().then(() => {
        if (!status.value?.running && launchMode.value !== modeBeforeConfig) {
          void loadData()
        }
      })
    }
  })

  watch(() => form.strategy_id, () => {
    if (formLocked.value) return
    enforceSupportedTimeframe()
    enforceSupportedSymbol()
    syncSelectedTriggerTimeframe()
    syncSelectedTriggerSymbol()
  })

  return {
    systemStore,
    status,
    orders,
    executionPlans,
    positions,
    scopedOrders,
    scopedExecutionPlans,
    equityHistory,
    scopedEquityHistory,
    executionLogRefreshError,
    executionLogs,
	    decisionDiagnostics,
	    currentDecisionDiagnostics,
	    decisionDiagnosticsScopeText,
	    autoDecisionDiagnosticsEnabled,
	    decisionDiagnosticsLoading,
	    decisionDiagnosticsRefreshSource,
	    decisionDiagnosticsError,
    triggerCandles,
    selectedTriggerSymbol,
    selectedTriggerTimeframe,
    triggerRangeDays,
    error,
    message,
    actionLoading,
    actionPhase,
    runParamModalOpen,
    form,
    formLocked,
    selectedStrategy,
    strategyOptions,
    launchMode,
    launchModeLabel,
    controlMode,
    controlModeLabel,
    actionBusyText,
    startDisabledReason,
    stopDisabledReason,
    startButtonText,
    stopButtonText,
    launchReadiness,
    activeDataMode,
    triggerTimeframe,
    triggerTimeframeOptions,
    triggerSymbolOptions,
    triggerRangeOptions,
    detailDataScope,
    runtimeRefreshNotice,
    liveTriggerMarkers,
    currentTriggerOrders,
    liveTriggerSubtitle,
    riskScopeNote,
    setInitialCapital,
    setStrategyId,
    setTriggerSymbol,
    setTriggerTimeframe,
    setTriggerRangeDays,
	    loadDecisionDiagnostics,
    openRunParamModal,
    submitRunParams,
    startStrategy,
    stopStrategy,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
