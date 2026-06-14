import { afterEach, beforeEach, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { defineComponent, h } from 'vue'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as liveApi from '@/api/live'
import * as marketApi from '@/api/market'
import * as marketRealtimeApi from '@/api/marketRealtime'
import * as tradingApi from '@/api/trading'
import { useLiveStrategyView } from '@/composables/useLiveStrategyView'
import { useSystemStore } from '@/stores/systemStore'
import type { StrategyMeta } from '@/types'
import LiveStrategyView from '@/views/LiveStrategyView.vue'
import {
  decisionDiagnostics,
  equityHistory,
  status,
} from '../fixtures/liveStrategy'

vi.mock('@/api/live', () => ({
  fetchAvailableStrategies: vi.fn(),
  fetchLiveStatus: vi.fn(),
  fetchLiveExecutionPlans: vi.fn(),
  fetchLiveOrders: vi.fn(),
  fetchLiveEquity: vi.fn(),
  fetchLiveExecutionLogs: vi.fn(),
  fetchDecisionDiagnostics: vi.fn(),
  startLiveStrategy: vi.fn(),
  stopLiveStrategy: vi.fn(),
}))

vi.mock('@/api/market', () => ({
  fetchCandles: vi.fn(),
}))

vi.mock('@/api/marketRealtime', () => ({
  subscribeCandle: vi.fn(),
  unsubscribeCandle: vi.fn(),
}))

vi.mock('@/api/trading', () => ({
  fetchPositions: vi.fn(),
}))

export type LiveStrategyTestPinia = ReturnType<typeof createPinia>

export const fetchAvailableStrategiesMock = vi.mocked(liveApi.fetchAvailableStrategies)
export const fetchLiveStatusMock = vi.mocked(liveApi.fetchLiveStatus)
export const fetchLiveExecutionPlansMock = vi.mocked(liveApi.fetchLiveExecutionPlans)
export const fetchLiveOrdersMock = vi.mocked(liveApi.fetchLiveOrders)
export const fetchLiveEquityMock = vi.mocked(liveApi.fetchLiveEquity)
const fetchLiveExecutionLogsMock = vi.mocked(liveApi.fetchLiveExecutionLogs)
const fetchDecisionDiagnosticsMock = vi.mocked(liveApi.fetchDecisionDiagnostics)
export const startLiveStrategyMock = vi.mocked(liveApi.startLiveStrategy)
const stopLiveStrategyMock = vi.mocked(liveApi.stopLiveStrategy)
export const fetchPositionsMock = vi.mocked(tradingApi.fetchPositions)
const fetchCandlesMock = vi.mocked(marketApi.fetchCandles)
const subscribeCandleMock = vi.mocked(marketRealtimeApi.subscribeCandle)
const unsubscribeCandleMock = vi.mocked(marketRealtimeApi.unsubscribeCandle)
const listenMock = vi.mocked(listen)

export function setupLiveStrategyViewHarness(assignPinia: (pinia: LiveStrategyTestPinia) => void) {
  beforeEach(() => {
    window.localStorage.clear()
    const pinia = createPinia()
    setActivePinia(pinia)
    assignPinia(pinia)
    useSystemStore().applySystemStatus({ okx: { mode: 'simulated' } })

    fetchAvailableStrategiesMock.mockResolvedValue([
      strategyMeta({ id: 'runtime_candidate_breakout_v1', name: 'Runtime Candidate Breakout V1' }),
    ])
    fetchLiveStatusMock.mockResolvedValue(status())
    fetchLiveExecutionPlansMock.mockResolvedValue([])
    fetchLiveOrdersMock.mockResolvedValue([])
    fetchLiveEquityMock.mockResolvedValue(equityHistory())
    fetchLiveExecutionLogsMock.mockResolvedValue([])
    fetchPositionsMock.mockResolvedValue([])
    fetchDecisionDiagnosticsMock.mockResolvedValue(decisionDiagnostics())
    startLiveStrategyMock.mockResolvedValue(status({ running: true, status: 'running' }))
    stopLiveStrategyMock.mockResolvedValue(status({ running: false, status: 'stopped' }))
    fetchCandlesMock.mockResolvedValue([])
    subscribeCandleMock.mockResolvedValue(undefined)
    unsubscribeCandleMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
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

export function mountLiveStrategyView(pinia: LiveStrategyTestPinia) {
  let view!: ReturnType<typeof useLiveStrategyView>
  const wrapper = mount(defineComponent({
    setup() {
      view = useLiveStrategyView()
      return () => h('div')
    },
  }), {
    global: {
      plugins: [pinia],
    },
  })
  return { view, wrapper }
}

export function mountLiveStrategyPage(pinia: LiveStrategyTestPinia) {
  return mount(LiveStrategyView, {
    global: {
      plugins: [pinia],
    },
  })
}
