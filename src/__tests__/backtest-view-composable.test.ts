import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, type Component } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as backtestApi from '@/api/backtest'
import { useBacktestView } from '@/composables/useBacktestView'
import type { BacktestResult, StrategyMeta, Timeframe } from '@/types'

vi.mock('@/api/backtest', () => ({
  fetchStrategies: vi.fn(),
  fetchBacktestHistory: vi.fn(),
  fetchBacktestDetail: vi.fn(),
  runBacktest: vi.fn(),
  deleteBacktestResult: vi.fn(),
  fetchBacktestProgress: vi.fn(),
}))

const fetchStrategiesMock = vi.mocked(backtestApi.fetchStrategies)
const fetchBacktestHistoryMock = vi.mocked(backtestApi.fetchBacktestHistory)
const runBacktestMock = vi.mocked(backtestApi.runBacktest)
const deleteBacktestResultMock = vi.mocked(backtestApi.deleteBacktestResult)
const fetchBacktestProgressMock = vi.mocked(backtestApi.fetchBacktestProgress)

describe('回测页策略运行', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    fetchStrategiesMock.mockResolvedValue([
      strategyMeta({ id: 'runtime_candidate_breakout_v1', name: 'Runtime Candidate Breakout V1' }),
    ])
    fetchBacktestHistoryMock.mockResolvedValue([])
    fetchBacktestProgressMock.mockResolvedValue(backtestProgress())
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('默认选择发现到的第一个运行策略', async () => {
    const view = mountBacktestComposable()
    await flushPromises()

    expect(view.strategyId.value).toBe('runtime_candidate_breakout_v1')
  })

  it('运行回测只提交策略 id，运行配置由策略文件 RUNTIME_CONFIG 决定', async () => {
    const view = mountBacktestComposable()
    await flushPromises()
    fetchBacktestHistoryMock.mockClear()
    runBacktestMock.mockResolvedValue(backtestResult())

    await view.run()

    expect(runBacktestMock).toHaveBeenCalledTimes(1)
    expect(runBacktestMock).toHaveBeenCalledWith('runtime_candidate_breakout_v1', {
      progress_id: expect.stringMatching(/^runtime_candidate_breakout_v1_/),
    })
    expect(fetchBacktestHistoryMock).toHaveBeenCalledTimes(1)
    expect(view.message.value).toContain('Runtime Candidate Breakout V1 回测运行完成')
    expect(view.runProgress.value?.status).toBe('completed')
    expect(view.runProgress.value?.progress).toBe(100)
  })

  it('运行回测时透传运行前选择的参数', async () => {
    const view = mountBacktestComposable()
    await flushPromises()
    runBacktestMock.mockResolvedValue(backtestResult({
      params: {
        strict_context_gating: true,
        leverage: 5,
      },
    }))

    await view.run({
      initial_capital: 2500,
      params: {
        strict_context_gating: true,
        leverage: 5,
      },
    })

    expect(runBacktestMock).toHaveBeenCalledTimes(1)
    expect(runBacktestMock).toHaveBeenCalledWith('runtime_candidate_breakout_v1', {
      initial_capital: 2500,
      params: {
        strict_context_gating: true,
        leverage: 5,
      },
      progress_id: expect.stringMatching(/^runtime_candidate_breakout_v1_/),
    })
  })

  it('删除历史回测后移除列表项并清空当前结果', async () => {
    const view = mountBacktestComposable()
    await flushPromises()
    deleteBacktestResultMock.mockResolvedValue({})
    const active = backtestResult({ result_id: 'result-a', strategy_name: 'A' })
    const remaining = backtestResult({ result_id: 'result-b', strategy_name: 'B' })
    view.store.history = [active, remaining]
    view.store.activeResult = active

    await view.deleteResult(active)

    expect(deleteBacktestResultMock).toHaveBeenCalledTimes(1)
    expect(deleteBacktestResultMock).toHaveBeenCalledWith('result-a')
    expect(view.store.history.map(item => item.result_id)).toEqual(['result-b'])
    expect(view.store.activeResult).toBeNull()
    expect(view.message.value).toBe('回测记录已删除')
    expect(view.error.value).toBeNull()
  })
})

function mountBacktestComposable() {
  let exposed!: ReturnType<typeof useBacktestView>
  const Harness: Component = defineComponent({
    setup() {
      exposed = useBacktestView()
      return () => h('div')
    },
  })
  mount(Harness, {
    global: {
      plugins: [createPinia()],
    },
  })
  return exposed
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
    description: '',
    ...overrides,
  }
}

function backtestResult(overrides: Partial<BacktestResult> = {}): BacktestResult {
  const timeframe = (overrides.timeframe ?? '15m') as Timeframe
  return {
    result_id: 'result',
    strategy_id: 'runtime_candidate_breakout_v1',
    strategy_name: 'Runtime Candidate Breakout V1',
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe,
    days: 10,
    initial_capital: 1000,
    final_equity: 1000,
    total_return_pct: 0,
    sharpe_ratio: 0,
    max_drawdown_pct: 0,
    win_rate_pct: 0,
    total_trades: 0,
    winning_trades: 0,
    losing_trades: 0,
    profit_factor: 0,
    trades: [],
    orders: [],
    fills: [],
    rejected_orders: [],
    trade_events_total: 0,
    trades_truncated: false,
    candles: [],
    indicators: {},
    equity_curve: [],
    created_at: '',
    ...overrides,
  }
}

function backtestProgress() {
  return {
    run_id: 'progress',
    strategy_id: 'runtime_candidate_breakout_v1',
    status: 'running' as const,
    stage: 'strategy',
    message: '执行策略',
    progress: 40,
    processed_candles: 10,
    total_candles: 25,
    started_at: '',
    updated_at: '',
  }
}
