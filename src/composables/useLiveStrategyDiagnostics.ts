import { computed, onScopeDispose, ref, watch } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import * as api from '@/api/live'
import type {
  LiveDecisionDiagnostics,
  LiveStrategyStatus,
  Timeframe,
  TradingMode,
} from '@/types'
import { describeError } from '@/utils/logger'
import {
  buildDiagnosticTarget,
  currentDecisionDiagnosticsForTarget,
  decisionDiagnosticsIsStale,
  decisionDiagnosticsMatchesTarget,
  decisionDiagnosticsMismatchText,
  decisionDiagnosticsPayload,
  diagnosticScopeText,
  diagnosticTargetKey as buildDiagnosticTargetKey,
  diagnosticsRefreshRequestKey as buildDiagnosticsRefreshRequestKey,
  latestRealtimeDiagnosticCandle,
  shouldRefreshDecisionDiagnosticsOnCandle,
} from '@/utils/liveStrategyDiagnostics'
import type { RealtimeTriggerCandle } from '@/utils/liveStrategyTrigger'

export type DecisionDiagnosticsRefreshSource = 'manual' | 'auto'

type LiveDiagnosticForm = {
  strategy_id: string
  symbol: string
  timeframe: Timeframe
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  params: Record<string, unknown>
}

type LiveStrategyDiagnosticsDeps = {
  status: Ref<LiveStrategyStatus | null>
  form: LiveDiagnosticForm
  controlMode: ComputedRef<TradingMode>
  selectedTriggerSymbol: Ref<string>
  triggerTimeframe: ComputedRef<Timeframe>
  latestRealtimeTriggerCandle: Ref<RealtimeTriggerCandle | null>
  defaultInitialCapital: number
  error: Ref<string | null>
}

export function useLiveStrategyDiagnostics({
  status,
  form,
  controlMode,
  selectedTriggerSymbol,
  triggerTimeframe,
  latestRealtimeTriggerCandle,
  defaultInitialCapital,
  error,
}: LiveStrategyDiagnosticsDeps) {
  const decisionDiagnostics = ref<LiveDecisionDiagnostics | null>(null)
  const decisionDiagnosticsTargetKey = ref('')
  const decisionDiagnosticsLoading = ref(false)
  const decisionDiagnosticsRefreshSource = ref<DecisionDiagnosticsRefreshSource | null>(null)
  const decisionDiagnosticsError = ref<string | null>(null)
  const decisionDiagnosticsArmed = ref(false)
  let decisionDiagnosticsSequence = 0
  let diagnosticsRefreshInFlight = false
  let diagnosticsRefreshInFlightKey = ''
  let diagnosticsRefreshPending = false

  const autoDecisionDiagnosticsEnabled = computed(() =>
    Boolean(status.value?.running && decisionDiagnosticsArmed.value)
  )
  const diagnosticTargetKey = computed(() => {
    return buildDiagnosticTargetKey(diagnosticTarget())
  })
  const currentDecisionDiagnostics = computed(() => {
    const target = diagnosticTarget()
    return currentDecisionDiagnosticsForTarget(
      decisionDiagnostics.value,
      decisionDiagnosticsTargetKey.value,
      diagnosticTargetKey.value,
      target,
    )
  })
  const decisionDiagnosticsScopeText = computed(() => {
    return diagnosticScopeText({
      target: diagnosticTarget(),
      loading: decisionDiagnosticsLoading.value,
      hasCurrentResult: Boolean(currentDecisionDiagnostics.value),
      emptyText: '选择策略和品种后评估当前决策',
      loadingPrefix: '正在评估',
    })
  })

  function diagnosticTarget() {
    return buildDiagnosticTarget({
      selectedSymbol: selectedTriggerSymbol.value,
      triggerTimeframe: triggerTimeframe.value,
      controlMode: controlMode.value,
      defaultInitialCapital,
      form,
    })
  }

  function scheduleDecisionDiagnosticsRefresh(confirmed = false) {
    if (!shouldRefreshDecisionDiagnosticsOnCandle(autoDecisionDiagnosticsEnabled.value, confirmed)) return
    void runScheduledDecisionDiagnosticsRefresh().catch((e) => {
      error.value = `决策诊断: ${describeError(e)}`
    })
  }

  async function runScheduledDecisionDiagnosticsRefresh() {
    if (!autoDecisionDiagnosticsEnabled.value) {
      clearScheduledDecisionDiagnosticsRefresh()
      return
    }
    const refreshKey = diagnosticsRefreshRequestKey()
    if (diagnosticsRefreshInFlight) {
      if (refreshKey !== diagnosticsRefreshInFlightKey) diagnosticsRefreshPending = true
      return
    }
    diagnosticsRefreshInFlight = true
    diagnosticsRefreshInFlightKey = refreshKey
    try {
      await loadDecisionDiagnostics({ source: 'auto' })
    } catch (e) {
      error.value = `决策诊断: ${describeError(e)}`
    } finally {
      diagnosticsRefreshInFlight = false
      diagnosticsRefreshInFlightKey = ''
      if (diagnosticsRefreshPending) {
        diagnosticsRefreshPending = false
        void runScheduledDecisionDiagnosticsRefresh()
      }
    }
  }

  function clearScheduledDecisionDiagnosticsRefresh() {
    diagnosticsRefreshPending = false
  }

  function clearAutoDecisionDiagnostics() {
    decisionDiagnosticsArmed.value = false
    clearScheduledDecisionDiagnosticsRefresh()
    decisionDiagnosticsError.value = null
  }

  function syncAutoDecisionDiagnosticsAfterDataLoad(currentStatus: LiveStrategyStatus | null) {
    if (autoDecisionDiagnosticsEnabled.value) {
      void loadDecisionDiagnostics({ source: 'auto' }).catch((e) => {
        error.value = `决策诊断: ${describeError(e)}`
      })
      return
    }
    if (!currentStatus?.running) decisionDiagnosticsArmed.value = false
    clearScheduledDecisionDiagnosticsRefresh()
    decisionDiagnosticsError.value = null
  }

  function diagnosticsRefreshRequestKey() {
    const target = diagnosticTarget()
    const latest = latestRealtimeDiagnosticCandle(latestRealtimeTriggerCandle.value, target)
    return buildDiagnosticsRefreshRequestKey(diagnosticTargetKey.value, latest)
  }

  async function loadDecisionDiagnostics(options: { source?: DecisionDiagnosticsRefreshSource } = {}) {
    const sequence = ++decisionDiagnosticsSequence
    const source = options.source ?? 'manual'
    const target = diagnosticTarget()
    if (!target.strategy_id || !target.symbol) {
      decisionDiagnostics.value = null
      decisionDiagnosticsTargetKey.value = ''
      decisionDiagnosticsError.value = null
      decisionDiagnosticsLoading.value = false
      decisionDiagnosticsRefreshSource.value = null
      return
    }
    const targetKey = diagnosticTargetKey.value
    if (decisionDiagnosticsIsStale(decisionDiagnostics.value, decisionDiagnosticsTargetKey.value, targetKey, target)) {
      decisionDiagnostics.value = null
      decisionDiagnosticsTargetKey.value = ''
    }
    decisionDiagnosticsLoading.value = true
    decisionDiagnosticsRefreshSource.value = source
    decisionDiagnosticsError.value = null
    const latestCandle = latestRealtimeDiagnosticCandle(latestRealtimeTriggerCandle.value, target)
    const payload = decisionDiagnosticsPayload(target, latestCandle)
    try {
      const diagnostics = await api.fetchDecisionDiagnostics(payload)
      if (sequence === decisionDiagnosticsSequence) {
        if (!decisionDiagnosticsMatchesTarget(diagnostics, target)) {
          decisionDiagnostics.value = null
          decisionDiagnosticsTargetKey.value = ''
          decisionDiagnosticsError.value = decisionDiagnosticsMismatchText(diagnostics)
          return
        }
        decisionDiagnostics.value = diagnostics
        decisionDiagnosticsTargetKey.value = targetKey
        decisionDiagnosticsArmed.value = true
      }
    } catch (e) {
      if (sequence === decisionDiagnosticsSequence) {
        if (decisionDiagnosticsIsStale(decisionDiagnostics.value, decisionDiagnosticsTargetKey.value, targetKey, target)) {
          decisionDiagnostics.value = null
          decisionDiagnosticsTargetKey.value = ''
        }
        decisionDiagnosticsError.value = describeError(e)
      }
      throw e
    } finally {
      if (sequence === decisionDiagnosticsSequence) {
        decisionDiagnosticsLoading.value = false
        decisionDiagnosticsRefreshSource.value = null
      }
    }
  }

  watch(diagnosticTargetKey, () => {
    decisionDiagnosticsArmed.value = false
    if (!autoDecisionDiagnosticsEnabled.value) {
      clearScheduledDecisionDiagnosticsRefresh()
      decisionDiagnosticsError.value = null
      return
    }
    void runScheduledDecisionDiagnosticsRefresh()
  })

  onScopeDispose(() => {
    decisionDiagnosticsSequence += 1
    diagnosticsRefreshInFlightKey = ''
    diagnosticsRefreshPending = false
  })

  return {
    decisionDiagnostics,
    currentDecisionDiagnostics,
    decisionDiagnosticsScopeText,
    autoDecisionDiagnosticsEnabled,
    decisionDiagnosticsLoading,
    decisionDiagnosticsRefreshSource,
    decisionDiagnosticsError,
    diagnosticTargetKey,
    diagnosticTarget,
    scheduleDecisionDiagnosticsRefresh,
    clearAutoDecisionDiagnostics,
    syncAutoDecisionDiagnosticsAfterDataLoad,
    loadDecisionDiagnostics,
  }
}
