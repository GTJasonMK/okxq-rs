import { computed, onScopeDispose, ref, watch } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import * as api from '@/api/live'
import * as realtimeApi from '@/api/marketRealtime'
import * as tradingApi from '@/api/trading'
import {
  useRealtimeSubscriptionBundle,
  type RealtimeSubscriptionSpec,
} from '@/composables/useRealtimeSubscriptionBundle'
import type {
  LiveExecutionPlan,
  LiveExecutionLogEntry,
  LiveOrder,
  LiveEquityHistory,
  LiveStrategyStatus,
  Position,
  StrategyMeta,
  TradingMode,
} from '@/types'
import { describeError } from '@/utils/logger'
import {
  LIVE_EQUITY_REFRESH_INTERVAL_MS,
  LIVE_RUNTIME_REFRESH_INTERVAL_MS,
} from '@/utils/liveStrategyControl'
import {
  compareEquitySnapshotsByTime,
  dailySummariesFromSnapshots,
  detailDataScopeText,
  equitySnapshotTimestamp,
  liveRuntimeDataScope,
  scopedLiveExecutionPlans,
  runtimeRefreshNoticeText,
  scopedLiveEquityHistory,
  scopedLiveOrders,
} from '@/utils/liveStrategyCore'
import { settledErrorMessages } from '@/utils/settled'

type RuntimeDataForm = {
  strategy_id: string
}

type LiveRuntimeDataDeps = {
  status: Ref<LiveStrategyStatus | null>
  executionPlans: Ref<LiveExecutionPlan[]>
  orders: Ref<LiveOrder[]>
  positions: Ref<Position[]>
  equityHistory: Ref<LiveEquityHistory | null>
  launchMode: ComputedRef<TradingMode>
  form: RuntimeDataForm
  strategies: Ref<StrategyMeta[]>
  error: Ref<string | null>
  setStrategies: (strategies: StrategyMeta[]) => void
  reconcileStrategyAvailability: () => void
  syncFormWithRunningStatus: (status: LiveStrategyStatus) => boolean
  applyStrategyRuntime: (strategyId: string) => boolean
  syncSelectedTriggerSymbol: () => void
  clearAutoDecisionDiagnostics: () => void
  syncAutoDecisionDiagnosticsAfterDataLoad: (status: LiveStrategyStatus | null) => void
}

const LIVE_EQUITY_HISTORY_LIMIT = 300
const LIVE_EXECUTION_LOG_LIMIT = 160
const LIVE_EXECUTION_LOG_REFRESH_INTERVAL_MS = 1_000
const LIVE_PRIVATE_EVENT_REFRESH_DELAY_MS = 250

export function useLiveStrategyRuntimeData({
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
}: LiveRuntimeDataDeps) {
  const executionLogs = ref<LiveExecutionLogEntry[]>([])
  const runtimeRefreshError = ref<string | null>(null)
  const equityRefreshError = ref<string | null>(null)
  const executionLogRefreshError = ref<string | null>(null)
  const lastRuntimeRefreshAt = ref(0)
  const lastEquityRefreshAt = ref(0)
  const lastExecutionLogRefreshAt = ref(0)
  let runtimeRefreshTimer: ReturnType<typeof window.setTimeout> | null = null
  let equityRefreshTimer: ReturnType<typeof window.setTimeout> | null = null
  let executionLogRefreshTimer: ReturnType<typeof window.setTimeout> | null = null
  let runtimeRefreshInFlight = false
  let equityRefreshInFlight = false
  let executionLogRefreshInFlight = false
  let runtimeRefreshRequestedDuringFlight = false
  let equityRefreshRequestedDuringFlight = false
  let disposed = false

  const activeDataMode = computed(() => status.value?.mode ?? launchMode.value)
  const activeRunId = computed(() => status.value?.run_id || '')
  const { error: privateRealtimeError } = useRealtimeSubscriptionBundle({
    source: () => [activeDataMode.value],
    listeners: [
      {
        eventName: 'okxq-private-account',
        handlePayload: (payload, ids) => {
          if (!sameRealtimeMode(payload, ids[0])) return
          requestEquityRefreshSoon()
        },
      },
      {
        eventName: 'okxq-private-order',
        handlePayload: (payload, ids) => {
          if (!sameRealtimeMode(payload, ids[0])) return
          requestRuntimeRefreshSoon()
        },
      },
      {
        eventName: 'okxq-private-algo-order',
        handlePayload: (payload, ids) => {
          if (!sameRealtimeMode(payload, ids[0])) return
          requestRuntimeRefreshSoon()
        },
      },
      {
        eventName: 'okxq-private-fill',
        handlePayload: (payload, ids) => {
          if (!sameRealtimeMode(payload, ids[0])) return
          requestRuntimeRefreshSoon()
          requestEquityRefreshSoon()
        },
      },
      {
        eventName: 'okxq-private-position',
        handlePayload: (payload, ids) => {
          if (!sameRealtimeMode(payload, ids[0])) return
          requestRuntimeRefreshSoon()
          requestEquityRefreshSoon()
        },
      },
    ],
    subscriptions: ([mode]) => livePrivateSubscriptionSpecs(mode as TradingMode),
  })
  const scopedExecutionPlans = computed(() => scopedLiveExecutionPlans(executionPlans.value, {
    mode: activeDataMode.value,
    runId: activeRunId.value,
  }))
  const scopedOrders = computed(() => scopedLiveOrders(orders.value, {
    mode: activeDataMode.value,
    runId: activeRunId.value,
  }))
  const hiddenOrderCount = computed(() => orders.value.length - scopedOrders.value.length)
  const scopedEquityHistory = computed<LiveEquityHistory | null>(() => scopedLiveEquityHistory(
    equityHistory.value,
    {
      mode: activeDataMode.value,
      runId: activeRunId.value,
    },
  ))
  const hiddenEquityByScope = computed(() =>
    Boolean(equityHistory.value && !scopedEquityHistory.value)
  )
  const detailDataScope = computed(() => detailDataScopeText({
    status: status.value,
    mode: activeDataMode.value,
    runId: activeRunId.value,
    hiddenOrderCount: hiddenOrderCount.value,
    hiddenEquityByScope: hiddenEquityByScope.value,
    scopedEquityHistory: scopedEquityHistory.value,
  }))
  const runtimeRefreshNotice = computed(() =>
    runtimeRefreshNoticeText(runtimeRefreshError.value, lastRuntimeRefreshAt.value)
  )

  async function loadData() {
    error.value = null
    const statusTask = api.fetchLiveStatus()
    const strategiesTask = api.fetchAvailableStrategies()
    const strategiesApplyTask = strategiesTask.then(
      (strategyRows) => {
        setStrategies(strategyRows)
        reconcileStrategyAvailability()
        return { status: 'fulfilled' as const, value: strategyRows }
      },
      (reason) => ({ status: 'rejected' as const, reason }),
    )
    const statusResult = await statusTask.then(
      (value) => ({ status: 'fulfilled' as const, value }),
      (reason) => ({ status: 'rejected' as const, reason }),
    )
    if (statusResult.status === 'fulfilled') {
      status.value = statusResult.value
      runtimeRefreshError.value = null
      lastRuntimeRefreshAt.value = Date.now()
    }
    const currentStatus = statusResult.status === 'fulfilled' ? statusResult.value : status.value
    const dataScope = liveRuntimeDataScope(currentStatus ?? null, launchMode.value)
    const [executionPlansResult, ordersResult, positionsResult, equityResult, executionLogsResult] = await Promise.allSettled([
      api.fetchLiveExecutionPlans({ limit: 200, mode: dataScope.mode, run_id: dataScope.runId }),
      api.fetchLiveOrders({ limit: 300, mode: dataScope.mode, run_id: dataScope.runId }),
      tradingApi.fetchPositions(dataScope.mode),
      api.fetchLiveEquity({ limit: 300, mode: dataScope.mode, run_id: dataScope.runId }),
      api.fetchLiveExecutionLogs({
        mode: dataScope.mode,
        run_id: dataScope.runId,
        limit: LIVE_EXECUTION_LOG_LIMIT,
      }),
    ])
    if (executionPlansResult.status === 'fulfilled') executionPlans.value = executionPlansResult.value
    if (ordersResult.status === 'fulfilled') orders.value = ordersResult.value
    if (positionsResult.status === 'fulfilled') positions.value = positionsResult.value
    if (equityResult.status === 'fulfilled') applyEquityHistory(equityResult.value)
    if (executionLogsResult.status === 'fulfilled') {
      executionLogs.value = executionLogsResult.value
      executionLogRefreshError.value = null
      lastExecutionLogRefreshAt.value = Date.now()
    }
    const strategiesResult = await strategiesApplyTask
    const syncedRunningForm = currentStatus?.running ? syncFormWithRunningStatus(currentStatus) : false
    if (!form.strategy_id && !syncedRunningForm) {
      const firstStrategy = strategies.value[0]
      if (firstStrategy) applyStrategyRuntime(firstStrategy.id)
    }
    syncSelectedTriggerSymbol()
    const errors = settledErrorMessages([
      { label: '状态', result: statusResult },
      { label: '退出计划', result: executionPlansResult },
      { label: '订单', result: ordersResult },
      { label: '持仓', result: positionsResult },
      { label: '权益', result: equityResult },
      { label: '执行日志', result: executionLogsResult },
      { label: '策略列表', result: strategiesResult },
    ], describeError)
    syncAutoDecisionDiagnosticsAfterDataLoad(currentStatus ?? null)
    if (errors.length > 0) error.value = errors.join('；')
    scheduleRuntimeRefresh()
    scheduleEquityRefresh()
    scheduleExecutionLogRefresh()
  }

  async function refreshRuntimeData() {
    if (disposed) return
    if (runtimeRefreshInFlight) {
      runtimeRefreshRequestedDuringFlight = true
      return
    }
    runtimeRefreshInFlight = true
    try {
      const current = await api.fetchLiveStatus()
      status.value = current
      if (!current.running) {
        clearAutoDecisionDiagnostics()
      }
      runtimeRefreshError.value = null
      lastRuntimeRefreshAt.value = Date.now()
      const dataScope = liveRuntimeDataScope(current, launchMode.value)
      const [executionPlansResult, ordersResult, positionsResult] = await Promise.allSettled([
        api.fetchLiveExecutionPlans({ limit: 200, mode: dataScope.mode, run_id: dataScope.runId }),
        api.fetchLiveOrders({ limit: 300, mode: dataScope.mode, run_id: dataScope.runId }),
        tradingApi.fetchPositions(dataScope.mode),
      ])
      if (executionPlansResult.status === 'fulfilled') executionPlans.value = executionPlansResult.value
      if (ordersResult.status === 'fulfilled') orders.value = ordersResult.value
      if (positionsResult.status === 'fulfilled') positions.value = positionsResult.value
      if (current.running) {
        syncFormWithRunningStatus(current)
      }
      if (!current.running && !form.strategy_id) {
        const firstStrategy = strategies.value[0]
        if (firstStrategy) applyStrategyRuntime(firstStrategy.id)
      }
      syncSelectedTriggerSymbol()
      const errors = settledErrorMessages([
        { label: '退出计划', result: executionPlansResult },
        { label: '订单', result: ordersResult },
        { label: '持仓', result: positionsResult },
      ], describeError)
      if (errors.length > 0) error.value = `运行状态刷新: ${errors.join('；')}`
      else if (error.value?.startsWith('运行状态刷新:')) error.value = null
    } catch (e) {
      runtimeRefreshError.value = describeError(e)
      error.value = `运行状态刷新: ${runtimeRefreshError.value}`
    } finally {
      runtimeRefreshInFlight = false
      if (runtimeRefreshRequestedDuringFlight) {
        runtimeRefreshRequestedDuringFlight = false
        scheduleRuntimeRefresh(LIVE_PRIVATE_EVENT_REFRESH_DELAY_MS)
      } else {
        scheduleRuntimeRefresh()
      }
    }
  }

  async function refreshEquityData() {
    if (disposed) return
    if (equityRefreshInFlight) {
      equityRefreshRequestedDuringFlight = true
      return
    }
    equityRefreshInFlight = true
    try {
      const dataScope = liveRuntimeDataScope(status.value, launchMode.value)
      const next = await api.fetchLiveEquity({
        limit: LIVE_EQUITY_HISTORY_LIMIT,
        mode: dataScope.mode,
        run_id: dataScope.runId,
      })
      applyEquityHistory(next)
      equityRefreshError.value = null
      lastEquityRefreshAt.value = Date.now()
      if (error.value?.startsWith('权益刷新:')) error.value = null
    } catch (e) {
      equityRefreshError.value = describeError(e)
      error.value = `权益刷新: ${equityRefreshError.value}`
    } finally {
      equityRefreshInFlight = false
      if (equityRefreshRequestedDuringFlight) {
        equityRefreshRequestedDuringFlight = false
        scheduleEquityRefresh(LIVE_PRIVATE_EVENT_REFRESH_DELAY_MS)
      } else {
        scheduleEquityRefresh()
      }
    }
  }

  async function refreshExecutionLogData() {
    if (executionLogRefreshInFlight || disposed) return
    executionLogRefreshInFlight = true
    try {
      const dataScope = liveRuntimeDataScope(status.value, launchMode.value)
      executionLogs.value = await api.fetchLiveExecutionLogs({
        mode: dataScope.mode,
        run_id: dataScope.runId,
        limit: LIVE_EXECUTION_LOG_LIMIT,
      })
      executionLogRefreshError.value = null
      lastExecutionLogRefreshAt.value = Date.now()
      if (error.value?.startsWith('执行日志刷新:')) error.value = null
    } catch (e) {
      executionLogRefreshError.value = describeError(e)
      error.value = `执行日志刷新: ${executionLogRefreshError.value}`
    } finally {
      executionLogRefreshInFlight = false
      scheduleExecutionLogRefresh()
    }
  }

  function requestRuntimeRefreshSoon() {
    scheduleRuntimeRefresh(LIVE_PRIVATE_EVENT_REFRESH_DELAY_MS)
  }

  function requestEquityRefreshSoon() {
    scheduleEquityRefresh(LIVE_PRIVATE_EVENT_REFRESH_DELAY_MS)
  }

  function scheduleRuntimeRefresh(delayMs = LIVE_RUNTIME_REFRESH_INTERVAL_MS) {
    if (runtimeRefreshTimer) {
      window.clearTimeout(runtimeRefreshTimer)
      runtimeRefreshTimer = null
    }
    if (disposed) return
    runtimeRefreshTimer = window.setTimeout(() => {
      runtimeRefreshTimer = null
      void refreshRuntimeData()
    }, delayMs)
  }

  function scheduleEquityRefresh(delayMs = LIVE_EQUITY_REFRESH_INTERVAL_MS) {
    if (equityRefreshTimer) {
      window.clearTimeout(equityRefreshTimer)
      equityRefreshTimer = null
    }
    if (disposed) return
    equityRefreshTimer = window.setTimeout(() => {
      equityRefreshTimer = null
      void refreshEquityData()
    }, delayMs)
  }

  function scheduleExecutionLogRefresh() {
    if (executionLogRefreshTimer) {
      window.clearTimeout(executionLogRefreshTimer)
      executionLogRefreshTimer = null
    }
    if (disposed) return
    executionLogRefreshTimer = window.setTimeout(() => {
      executionLogRefreshTimer = null
      void refreshExecutionLogData()
    }, LIVE_EXECUTION_LOG_REFRESH_INTERVAL_MS)
  }

  function applyEquityHistory(next: LiveEquityHistory) {
    equityHistory.value = mergeLiveEquityHistory(equityHistory.value, next, LIVE_EQUITY_HISTORY_LIMIT)
  }

  watch(privateRealtimeError, (subscriptionError) => {
    if (!subscriptionError) return
    equityRefreshError.value = subscriptionError
    runtimeRefreshError.value = subscriptionError
  })

  onScopeDispose(() => {
    disposed = true
    if (runtimeRefreshTimer) {
      window.clearTimeout(runtimeRefreshTimer)
      runtimeRefreshTimer = null
    }
    if (equityRefreshTimer) {
      window.clearTimeout(equityRefreshTimer)
      equityRefreshTimer = null
    }
    if (executionLogRefreshTimer) {
      window.clearTimeout(executionLogRefreshTimer)
      executionLogRefreshTimer = null
    }
  })

  return {
    activeDataMode,
    activeRunId,
    detailDataScope,
    equityHistory,
    equityRefreshError,
    executionLogRefreshError,
    executionLogs,
    executionPlans,
    hiddenEquityByScope,
    hiddenOrderCount,
    lastExecutionLogRefreshAt,
    lastEquityRefreshAt,
    lastRuntimeRefreshAt,
    loadData,
    orders,
    positions,
    refreshRuntimeData,
    runtimeRefreshError,
    runtimeRefreshNotice,
    scopedEquityHistory,
    scopedExecutionPlans,
    scopedOrders,
    status,
  }
}

function mergeLiveEquityHistory(
  current: LiveEquityHistory | null,
  next: LiveEquityHistory,
  limit: number,
): LiveEquityHistory {
  if (!isOkxAccountEquityHistory(next)) return next
  const baseSnapshots = current && sameEquityStream(current, next)
    ? current.snapshots
    : []
  const snapshots = uniqueEquitySnapshots([...baseSnapshots, ...next.snapshots])
    .sort(compareEquitySnapshotsByTime)
    .slice(-limit)
  return {
    ...next,
    count: snapshots.length,
    snapshots,
    daily: dailySummariesFromSnapshots(snapshots),
  }
}

function sameEquityStream(left: LiveEquityHistory, right: LiveEquityHistory): boolean {
  return left.mode === right.mode
    && left.run_id === right.run_id
    && isOkxAccountEquityHistory(left)
}

function uniqueEquitySnapshots(
  snapshots: LiveEquityHistory['snapshots'],
): LiveEquityHistory['snapshots'] {
  const byKey = new Map<string, LiveEquityHistory['snapshots'][number]>()
  for (const snapshot of snapshots) {
    const timestamp = equitySnapshotTimestamp(snapshot)
    if (timestamp <= 0 || !Number.isFinite(snapshot.equity)) continue
    byKey.set(`${snapshot.mode}:${snapshot.run_id}:${equityHistoryStreamSource(snapshot.source)}:${timestamp}:${snapshot.id}`, snapshot)
  }
  return Array.from(byKey.values())
}

function isOkxAccountEquityHistory(history: LiveEquityHistory): boolean {
  return equityHistoryStreamSource(history.source) === 'okx_account'
    || history.snapshots.some(snapshot => equityHistoryStreamSource(snapshot.source) === 'okx_account')
}

function equityHistoryStreamSource(source: string | undefined) {
  return source === 'okx_account_balance' || source === 'okx_account_ws_cache'
    ? 'okx_account'
    : source || ''
}

function livePrivateSubscriptionSpecs(mode: TradingMode): RealtimeSubscriptionSpec[] {
  return [
    {
      key: `live-private-account:${mode}`,
      subscribe: () => realtimeApi.subscribeAccount(mode),
      unsubscribe: () => realtimeApi.unsubscribeAccount(mode),
    },
    {
      key: `live-private-orders:${mode}`,
      subscribe: () => realtimeApi.subscribeOrders(mode),
      unsubscribe: () => realtimeApi.unsubscribeOrders(mode),
    },
    {
      key: `live-private-algo-orders:${mode}`,
      subscribe: () => realtimeApi.subscribeAlgoOrders(mode),
      unsubscribe: () => realtimeApi.unsubscribeAlgoOrders(mode),
    },
    {
      key: `live-private-fills:${mode}`,
      subscribe: () => realtimeApi.subscribeFills(mode),
      unsubscribe: () => realtimeApi.unsubscribeFills(mode),
    },
    {
      key: `live-private-positions:${mode}`,
      subscribe: () => realtimeApi.subscribePositions(mode),
      unsubscribe: () => realtimeApi.unsubscribePositions(mode),
    },
  ]
}

function sameRealtimeMode(payload: Record<string, unknown>, activeMode: string | undefined): boolean {
  const payloadMode = normalizeRealtimeMode(payload.mode)
  const currentMode = normalizeRealtimeMode(activeMode)
  if (hasRealtimeMode(payload) && !payloadMode) return false
  return !payloadMode || !currentMode || payloadMode === currentMode
}

function normalizeRealtimeMode(value: unknown): TradingMode | '' {
  const mode = String(value || '').trim().toLowerCase()
  if (mode === 'live') return 'live'
  if (mode === 'simulated') return 'simulated'
  return ''
}

function hasRealtimeMode(payload: Record<string, unknown>) {
  return Object.prototype.hasOwnProperty.call(payload, 'mode')
}
