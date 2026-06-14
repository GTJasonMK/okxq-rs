import { computed, defineComponent, h, reactive, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { listen } from '@tauri-apps/api/event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as liveApi from '@/api/live'
import * as marketRealtime from '@/api/marketRealtime'
import * as tradingApi from '@/api/trading'
import { useLiveStrategyRuntimeData } from '@/composables/useLiveStrategyRuntimeData'
import type {
  LiveExecutionPlan,
  LiveOrder,
  LiveEquityHistory,
  LiveStrategyStatus,
  Position,
  StrategyMeta,
  TradingMode,
} from '@/types'
import {
  LIVE_EQUITY_REFRESH_INTERVAL_MS,
  LIVE_RUNTIME_REFRESH_INTERVAL_MS,
} from '@/utils/liveStrategyControl'
import { deferred, equityHistory, liveExecutionPlan, liveOrder, status } from './fixtures/liveStrategy'

vi.mock('@/api/live', () => ({
  fetchAvailableStrategies: vi.fn(),
  fetchLiveStatus: vi.fn(),
  fetchLiveExecutionPlans: vi.fn(),
  fetchLiveOrders: vi.fn(),
  fetchLiveEquity: vi.fn(),
  fetchLiveExecutionLogs: vi.fn(),
}))

vi.mock('@/api/trading', () => ({
  fetchPositions: vi.fn(),
}))

vi.mock('@/api/marketRealtime', () => ({
  subscribeAccount: vi.fn(),
  unsubscribeAccount: vi.fn(),
  subscribeOrders: vi.fn(),
  unsubscribeOrders: vi.fn(),
  subscribeAlgoOrders: vi.fn(),
  unsubscribeAlgoOrders: vi.fn(),
  subscribeFills: vi.fn(),
  unsubscribeFills: vi.fn(),
  subscribePositions: vi.fn(),
  unsubscribePositions: vi.fn(),
}))

const listenMock = vi.mocked(listen)
const fetchAvailableStrategiesMock = vi.mocked(liveApi.fetchAvailableStrategies)
const fetchLiveStatusMock = vi.mocked(liveApi.fetchLiveStatus)
const fetchLiveExecutionPlansMock = vi.mocked(liveApi.fetchLiveExecutionPlans)
const fetchLiveOrdersMock = vi.mocked(liveApi.fetchLiveOrders)
const fetchLiveEquityMock = vi.mocked(liveApi.fetchLiveEquity)
const fetchLiveExecutionLogsMock = vi.mocked(liveApi.fetchLiveExecutionLogs)
const fetchPositionsMock = vi.mocked(tradingApi.fetchPositions)
const subscribeAccountMock = vi.mocked(marketRealtime.subscribeAccount)
const unsubscribeAccountMock = vi.mocked(marketRealtime.unsubscribeAccount)
const subscribeOrdersMock = vi.mocked(marketRealtime.subscribeOrders)
const unsubscribeOrdersMock = vi.mocked(marketRealtime.unsubscribeOrders)
const subscribeAlgoOrdersMock = vi.mocked(marketRealtime.subscribeAlgoOrders)
const unsubscribeAlgoOrdersMock = vi.mocked(marketRealtime.unsubscribeAlgoOrders)
const subscribeFillsMock = vi.mocked(marketRealtime.subscribeFills)
const unsubscribeFillsMock = vi.mocked(marketRealtime.unsubscribeFills)
const subscribePositionsMock = vi.mocked(marketRealtime.subscribePositions)
const unsubscribePositionsMock = vi.mocked(marketRealtime.unsubscribePositions)

type RealtimeListener = (event: { payload: Record<string, unknown> }) => void

describe('useLiveStrategyRuntimeData', () => {
  beforeEach(() => {
    fetchAvailableStrategiesMock.mockResolvedValue([strategyMeta()])
    fetchLiveStatusMock.mockResolvedValue(status())
    fetchLiveExecutionPlansMock.mockResolvedValue([])
    fetchLiveOrdersMock.mockResolvedValue([])
    fetchLiveEquityMock.mockResolvedValue(equityHistory())
    fetchLiveExecutionLogsMock.mockResolvedValue([])
    fetchPositionsMock.mockResolvedValue([])
    subscribeAccountMock.mockResolvedValue(undefined)
    unsubscribeAccountMock.mockResolvedValue(undefined)
    subscribeOrdersMock.mockResolvedValue(undefined)
    unsubscribeOrdersMock.mockResolvedValue(undefined)
    subscribeAlgoOrdersMock.mockResolvedValue(undefined)
    unsubscribeAlgoOrdersMock.mockResolvedValue(undefined)
    subscribeFillsMock.mockResolvedValue(undefined)
    unsubscribeFillsMock.mockResolvedValue(undefined)
    subscribePositionsMock.mockResolvedValue(undefined)
    unsubscribePositionsMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('applies strategies before waiting for slow runtime status', async () => {
    const slowStatus = deferred<LiveStrategyStatus>()
    fetchLiveStatusMock.mockReturnValueOnce(slowStatus.promise)
    const harness = mountRuntimeData()

    const loading = harness.runtime.loadData()
    await flushPromises()

    expect(harness.setStrategies).toHaveBeenCalledWith([expect.objectContaining({ id: 'strategy-a' })])
    expect(harness.reconcileStrategyAvailability).toHaveBeenCalled()
    expect(fetchLiveOrdersMock).not.toHaveBeenCalled()

    slowStatus.resolve(status({ running: false, status: 'stopped' }))
    await loading

    expect(fetchLiveExecutionPlansMock).toHaveBeenCalledWith({
      limit: 200,
      mode: 'simulated',
      run_id: 'run',
    })
    expect(fetchLiveOrdersMock).toHaveBeenCalledWith({
      limit: 300,
      mode: 'simulated',
      run_id: 'run',
    })
    expect(fetchPositionsMock).toHaveBeenCalledWith('simulated')
    expect(fetchLiveExecutionLogsMock).toHaveBeenCalledWith({
      mode: 'simulated',
      run_id: 'run',
      limit: 160,
    })
    expect(harness.syncSelectedTriggerSymbol).toHaveBeenCalled()

    harness.wrapper.unmount()
  })

  it('refreshes scoped runtime records and syncs running form state', async () => {
    vi.useFakeTimers()
    const runningStatus = status({
      running: true,
      status: 'running',
      mode: 'live',
      run_id: 'run-live',
      strategy_id: 'strategy-live',
    })
    const order = liveOrder({ mode: 'live', run_id: 'run-live' })
    const plan = liveExecutionPlan({ mode: 'live', entry_run_id: 'run-live' })
    const currentPosition = position({ inst_id: 'ETH-USDT-SWAP', pos_side: 'long', pos: 2 })
    fetchLiveStatusMock.mockResolvedValueOnce(runningStatus)
    fetchLiveExecutionPlansMock.mockResolvedValueOnce([plan])
    fetchLiveOrdersMock.mockResolvedValueOnce([order])
    fetchPositionsMock.mockResolvedValueOnce([currentPosition])
    const harness = mountRuntimeData()

    await harness.runtime.refreshRuntimeData()

    expect(harness.status.value).toMatchObject({
      running: true,
      mode: 'live',
      run_id: 'run-live',
      strategy_id: 'strategy-live',
    })
    expect(fetchLiveExecutionPlansMock).toHaveBeenCalledWith({
      limit: 200,
      mode: 'live',
      run_id: 'run-live',
    })
    expect(fetchLiveOrdersMock).toHaveBeenCalledWith({
      limit: 300,
      mode: 'live',
      run_id: 'run-live',
    })
    expect(fetchLiveEquityMock).not.toHaveBeenCalled()
    expect(fetchPositionsMock).toHaveBeenCalledWith('live')
    expect(harness.runtime.positions.value).toEqual([currentPosition])
    expect(harness.runtime.scopedExecutionPlans.value).toEqual([plan])
    expect(harness.runtime.scopedOrders.value).toEqual([order])
    expect(harness.syncFormWithRunningStatus).toHaveBeenCalledWith(runningStatus)

    harness.wrapper.unmount()
  })

  it('stays subscribed to OKX account equity while strategy is stopped', async () => {
    vi.useFakeTimers()
    fetchLiveStatusMock.mockResolvedValue(status({ running: false, status: 'stopped', run_id: '' }))
    fetchLiveEquityMock
      .mockResolvedValueOnce(okxAccountEquityHistory(1_780_000_000_000, 1000))
      .mockResolvedValueOnce(okxAccountEquityHistory(1_780_000_005_000, 1005))
    fetchPositionsMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([position({ inst_id: 'BTC-USDT-SWAP', pos_side: 'short', pos: -1 })])
    const harness = mountRuntimeData()

    await harness.runtime.loadData()
    expect(harness.runtime.scopedEquityHistory.value?.snapshots.map(row => row.equity)).toEqual([1000])

    await vi.advanceTimersByTimeAsync(LIVE_EQUITY_REFRESH_INTERVAL_MS)
    await flushPromises()

    expect(fetchLiveEquityMock).toHaveBeenCalledTimes(2)
    expect(fetchPositionsMock).toHaveBeenCalledTimes(1)
    expect(fetchLiveEquityMock).toHaveBeenLastCalledWith({
      limit: 300,
      mode: 'simulated',
      run_id: '',
    })
    expect(harness.runtime.scopedEquityHistory.value?.source).toBe('okx_account_balance')
    expect(harness.runtime.scopedEquityHistory.value?.snapshots.map(row => row.equity)).toEqual([1000, 1005])
    expect(harness.runtime.scopedEquityHistory.value?.daily[0]?.snapshot_count).toBe(2)

    await vi.advanceTimersByTimeAsync(LIVE_RUNTIME_REFRESH_INTERVAL_MS - LIVE_EQUITY_REFRESH_INTERVAL_MS)
    await flushPromises()

    expect(fetchPositionsMock).toHaveBeenCalledTimes(2)

    harness.wrapper.unmount()
  })

  it('subscribes all private websocket channels for the active live data mode', async () => {
    fetchLiveStatusMock.mockResolvedValue(status({ running: false, status: 'stopped', run_id: '' }))
    const harness = mountRuntimeData()

    await harness.runtime.loadData()
    await flushPromises()

    expect(subscribeAccountMock).toHaveBeenCalledWith('simulated')
    expect(subscribeOrdersMock).toHaveBeenCalledWith('simulated')
    expect(subscribeAlgoOrdersMock).toHaveBeenCalledWith('simulated')
    expect(subscribeFillsMock).toHaveBeenCalledWith('simulated')
    expect(subscribePositionsMock).toHaveBeenCalledWith('simulated')

    harness.wrapper.unmount()
  })

  it('switches all private websocket subscriptions when runtime mode changes', async () => {
    const liveStatus = status({
      running: true,
      status: 'running',
      mode: 'live',
      run_id: 'run-live',
    })
    fetchLiveStatusMock.mockResolvedValueOnce(liveStatus)
    const harness = mountRuntimeData()

    await flushPromises()
    expect(subscribeAccountMock).toHaveBeenCalledWith('simulated')

    await harness.runtime.refreshRuntimeData()
    await flushPromises()

    expect(unsubscribeAccountMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeOrdersMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeAlgoOrdersMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeFillsMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribePositionsMock).toHaveBeenCalledWith('simulated')
    expect(subscribeAccountMock).toHaveBeenCalledWith('live')
    expect(subscribeOrdersMock).toHaveBeenCalledWith('live')
    expect(subscribeAlgoOrdersMock).toHaveBeenCalledWith('live')
    expect(subscribeFillsMock).toHaveBeenCalledWith('live')
    expect(subscribePositionsMock).toHaveBeenCalledWith('live')

    harness.wrapper.unmount()
    await flushPromises()

    expect(unsubscribeAccountMock).toHaveBeenCalledWith('live')
    expect(unsubscribeOrdersMock).toHaveBeenCalledWith('live')
    expect(unsubscribeAlgoOrdersMock).toHaveBeenCalledWith('live')
    expect(unsubscribeFillsMock).toHaveBeenCalledWith('live')
    expect(unsubscribePositionsMock).toHaveBeenCalledWith('live')
  })

  it('refreshes live runtime data shortly after private order events', async () => {
    vi.useFakeTimers()
    const listeners = captureRealtimeListeners()
    const runningStatus = status({
      running: true,
      status: 'running',
      mode: 'simulated',
      run_id: 'run-private-event',
    })
    const order = liveOrder({ mode: 'simulated', run_id: 'run-private-event' })
    fetchLiveStatusMock.mockResolvedValue(runningStatus)
    fetchLiveOrdersMock.mockResolvedValue([order])
    const harness = mountRuntimeData()

    await waitFor(() => Boolean(listeners['okxq-private-order']))

    listeners['okxq-private-order']?.({
      payload: {
        mode: 'simulated',
        ord_id: 'order-ws',
        inst_id: 'BTC-USDT-SWAP',
        state: 'live',
      },
    })
    await vi.advanceTimersByTimeAsync(249)
    expect(fetchLiveStatusMock).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)
    await flushPromises()

    expect(fetchLiveStatusMock).toHaveBeenCalledTimes(1)
    expect(fetchLiveOrdersMock).toHaveBeenCalledWith({
      limit: 300,
      mode: 'simulated',
      run_id: 'run-private-event',
    })
    expect(fetchPositionsMock).toHaveBeenCalledWith('simulated')
    expect(harness.runtime.scopedOrders.value).toEqual([order])

    harness.wrapper.unmount()
  })

  it('refreshes equity shortly after private account events and ignores other modes', async () => {
    vi.useFakeTimers()
    const listeners = captureRealtimeListeners()
    fetchLiveEquityMock.mockResolvedValue(okxAccountEquityHistory(1_780_000_010_000, 1008))
    const harness = mountRuntimeData()

    await waitFor(() => Boolean(listeners['okxq-private-account']))

    listeners['okxq-private-account']?.({
      payload: { mode: 'live', account: {} },
    })
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    expect(fetchLiveEquityMock).not.toHaveBeenCalled()

    listeners['okxq-private-account']?.({
      payload: { mode: 'demo', account: {} },
    })
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    expect(fetchLiveEquityMock).not.toHaveBeenCalled()

    listeners['okxq-private-account']?.({
      payload: { mode: 'simulated', account: {} },
    })
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()

    expect(fetchLiveEquityMock).toHaveBeenCalledWith({
      limit: 300,
      mode: 'simulated',
      run_id: '',
    })

    harness.wrapper.unmount()
  })

  it('keeps refresh failures visible through error and notice text', async () => {
    fetchLiveStatusMock.mockRejectedValueOnce(new Error('status offline'))
    const harness = mountRuntimeData()

    await harness.runtime.refreshRuntimeData()

    expect(harness.error.value).toContain('运行状态刷新: status offline')
    expect(harness.runtime.runtimeRefreshError.value).toBe('status offline')
    expect(harness.runtime.runtimeRefreshNotice.value).toContain('status offline')

    harness.wrapper.unmount()
  })
})

function mountRuntimeData() {
  const statusRef = ref<LiveStrategyStatus | null>(null)
  const executionPlans = ref<LiveExecutionPlan[]>([])
  const orders = ref<LiveOrder[]>([])
  const positions = ref<Position[]>([])
  const equity = ref<LiveEquityHistory | null>(null)
  const strategies = ref<StrategyMeta[]>([])
  const form = reactive({ strategy_id: '' })
  const error = ref<string | null>(null)
  const setStrategies = vi.fn((rows: StrategyMeta[]) => {
    strategies.value = rows
  })
  const reconcileStrategyAvailability = vi.fn()
  const syncFormWithRunningStatus = vi.fn((current: LiveStrategyStatus) => {
    form.strategy_id = current.strategy_id
    return true
  })
  const applyStrategyRuntime = vi.fn((strategyId: string) => {
    form.strategy_id = strategyId
    return true
  })
  const syncSelectedTriggerSymbol = vi.fn()
  const clearAutoDecisionDiagnostics = vi.fn()
  const syncAutoDecisionDiagnosticsAfterDataLoad = vi.fn()
  let runtime!: ReturnType<typeof useLiveStrategyRuntimeData>
  const wrapper = mount(defineComponent({
    setup() {
      runtime = useLiveStrategyRuntimeData({
        status: statusRef,
        executionPlans,
        orders,
        positions,
        equityHistory: equity,
        launchMode: computed<TradingMode>(() => 'simulated'),
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
      return () => h('div')
    },
  }))
  return {
    applyStrategyRuntime,
    clearAutoDecisionDiagnostics,
    error,
    form,
    reconcileStrategyAvailability,
    runtime,
    setStrategies,
    status: statusRef,
    executionPlans,
    positions,
    strategies,
    syncAutoDecisionDiagnosticsAfterDataLoad,
    syncFormWithRunningStatus,
    syncSelectedTriggerSymbol,
    wrapper,
  }
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy-a',
    name: 'Strategy A',
    description: '',
    runtime: {
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      risk_timeframe: '1m',
      initial_capital: 1000,
      position_size: 0.25,
      stop_loss: 0,
      take_profit: 0,
      check_interval: 60,
      mode: 'simulated',
      params: {},
    },
    visualization: {},
    decision_contract: {},
    ...overrides,
  }
}

function position(overrides: Partial<Position> = {}): Position {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    pos_side: 'long',
    pos: 1,
    mgn_mode: 'cross',
    avg_px: 100,
    upl: 0,
    upl_ratio: 0,
    lever: 3,
    margin: 50,
    mark_px: 101,
    ...overrides,
  }
}

function okxAccountEquityHistory(timestamp: number, equity: number): LiveEquityHistory {
  return equityHistory({
    run_id: '',
    mode: 'simulated',
    count: 1,
    source: 'okx_account_balance',
    pnl_available: false,
    snapshots: [{
      id: 0,
      run_id: '',
      strategy_id: '',
      strategy_name: '',
      symbol: '',
      inst_id: '',
      timeframe: '1H',
      inst_type: 'SPOT',
      mode: 'simulated',
      timestamp: timestamp,
      time: new Date(timestamp).toISOString(),
      trading_day: '2026-05-28',
      price: 0,
      position_side: 'flat',
      entry_price: 0,
      quantity: 0,
      initial_capital: equity,
      day_start_equity: equity,
      equity,
      realized_pnl: 0,
      unrealized_pnl: 0,
      total_pnl: 0,
      total_pnl_pct: 0,
      today_pnl: 0,
      today_pnl_pct: 0,
      created_at: timestamp,
      pnl_available: false,
      source: 'okx_account_balance',
    }],
    daily: [],
  })
}

function captureRealtimeListeners() {
  const listeners: Record<string, RealtimeListener> = {}
  listenMock.mockImplementation(async (event, handler) => {
    listeners[String(event)] = handler as RealtimeListener
    return () => {}
  })
  return listeners
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 10; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
