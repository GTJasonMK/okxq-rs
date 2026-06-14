import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, type Component } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as backtestApi from '@/api/backtest'
import * as marketApi from '@/api/market'
import BacktestView from '@/views/BacktestView.vue'
import { useBacktestStore } from '@/stores/backtestStore'
import type { BacktestEquitySnapshot, BacktestProgress, BacktestResult, BacktestTrade, LiveOrder, StrategyMeta, Timeframe } from '@/types'

vi.mock('@/api/backtest', () => ({
  fetchStrategies: vi.fn(),
  fetchBacktestHistory: vi.fn(),
  fetchBacktestDetail: vi.fn(),
  runBacktest: vi.fn(),
  deleteBacktestResult: vi.fn(),
  fetchBacktestProgress: vi.fn(),
}))

vi.mock('@/api/market', () => ({
  fetchDefaultWatchScope: vi.fn(),
}))

const fetchStrategiesMock = vi.mocked(backtestApi.fetchStrategies)
const fetchBacktestHistoryMock = vi.mocked(backtestApi.fetchBacktestHistory)
const fetchBacktestDetailMock = vi.mocked(backtestApi.fetchBacktestDetail)
const fetchDefaultWatchScopeMock = vi.mocked(marketApi.fetchDefaultWatchScope)
const runBacktestMock = vi.mocked(backtestApi.runBacktest)
const deleteBacktestResultMock = vi.mocked(backtestApi.deleteBacktestResult)
const fetchBacktestProgressMock = vi.mocked(backtestApi.fetchBacktestProgress)

describe('回测页组件渲染', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    fetchDefaultWatchScopeMock.mockResolvedValue({
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
    } as never)
    fetchStrategiesMock.mockResolvedValue([
      strategyMeta({ id: 'multi_timeframe_dual_v12', name: 'V20' }),
    ])
    fetchBacktestHistoryMock.mockResolvedValue([])
    fetchBacktestProgressMock.mockResolvedValue(backtestProgress())
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('默认用余额K线作为回测结果主视图', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({
      equity_snapshots: [
        snapshot({ time: 1000, equity: 1000 }),
        snapshot({ time: 2000, equity: 1010 }),
      ],
      trades: [
        trade({ timestamp: 2000, datetime: '2026-05-10T00:00:00.000Z', action: 'open', side: 'buy', pos_side: 'long', price: 100, quantity: 1 }),
      ],
    })
    await nextTick()

    const activeTab = wrapper.find('.vb-chart-tabs .vb-tab.active')
    expect(activeTab.text()).toContain('余额K线')
    expect(wrapper.findAll('.vb-chart-tabs .vb-tab')).toHaveLength(3)
    expect(wrapper.findAll('.vb-chart-tabs .vb-tab').some(tab => tab.text().includes('币种收益'))).toBe(true)
    expect(wrapper.findAll('.vb-chart-tabs .vb-tab').some(tab => tab.text().includes('订单明细'))).toBe(true)
    expect(wrapper.findAll('.vb-chart-tabs .vb-tab').some(tab => tab.text().includes('价格参考'))).toBe(false)
    expect(wrapper.findAll('.vb-data-tabs .vb-tab')).toHaveLength(2)
    expect(wrapper.findAll('.vb-data-tabs .vb-tab').some(tab => tab.text().includes('回测概览'))).toBe(true)
    expect(wrapper.findAll('.vb-data-tabs .vb-tab').some(tab => tab.text().includes('订单分布'))).toBe(true)
    expect(wrapper.findAll('.vb-data-tabs .vb-tab').some(tab => tab.text().includes('交易明细'))).toBe(false)
    expect(wrapper.findAll('.vb-data-tabs .vb-tab').some(tab => tab.text().includes('币种收益'))).toBe(false)
    const chart = wrapper.find('.stub-EquityCandleChart')
    expect(chart.text()).toContain('2 snapshots')
    expect(chart.text()).toContain('1 trades')
    expect(wrapper.find('.stub-BacktestResultCard').exists()).toBe(true)
    expect(wrapper.find('.event-list').exists()).toBe(false)
  })

  it('跨币种回测在余额K线保留全量事件并移除价格参考主图入口', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({
      symbol: 'BTC-USDT-SWAP',
      candles: [
        {
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          timestamp: 1000,
          open: 100,
          high: 110,
          low: 95,
          close: 105,
          volume: 1,
        },
      ],
      equity_snapshots: [
        snapshot({ time: 1000, equity: 1000, position_side: 'portfolio' }),
        snapshot({ time: 2000, equity: 1015, position_side: 'portfolio' }),
      ],
      trades: [
        trade({ symbol: 'BTC-USDT-SWAP', timestamp: 1000, action: 'open', side: 'buy', pos_side: 'long', price: 100 }),
        trade({ symbol: 'ETH-USDT-SWAP', timestamp: 2000, action: 'open', side: 'sell', pos_side: 'short', price: 50 }),
      ],
    })
    await nextTick()

    const equityChart = wrapper.find('.stub-EquityCandleChart')
    expect(equityChart.text()).toContain('2 snapshots')
    expect(equityChart.text()).toContain('2 trades')
    expect(wrapper.find('.detail-grid').exists()).toBe(false)
    expect(wrapper.find('.event-list').exists()).toBe(false)
    expect(wrapper.findAll('.vb-chart-tabs .vb-tab').some(tab => tab.text().includes('价格参考'))).toBe(false)
  })

  it('余额K线周期默认跟随回测结果并补充非固定周期选项', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    const start = Date.UTC(2026, 0, 1, 0, 0, 0)
    store.activeResult = backtestResult({
      timeframe: '5m',
      equity_snapshots: [
        snapshot({ time: start, equity: 1000 }),
        snapshot({ time: start + 5 * 60_000, equity: 1010 }),
      ],
    })
    await nextTick()

    const timeframeSelect = wrapper.get('.theme-select-stub')
    expect((timeframeSelect.element as HTMLSelectElement).value).toBe('5m')
    expect(timeframeSelect.findAll('option').map(option => option.attributes('value'))).toContain('5m')
    expect(wrapper.find('.stub-EquityCandleChart').text()).toContain('5m')
    expect(wrapper.find('.stub-EquityCandleChart').text()).toContain('2 candles')
  })

  it('切换历史回测结果时余额K线周期回到新结果的真实周期', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({ timeframe: '5m' })
    await nextTick()

    await wrapper.get('.theme-select-stub').setValue('1H')
    await nextTick()
    expect((wrapper.get('.theme-select-stub').element as HTMLSelectElement).value).toBe('1H')

    store.activeResult = backtestResult({ result_id: 'three-min', timeframe: '3m' })
    await nextTick()

    expect((wrapper.get('.theme-select-stub').element as HTMLSelectElement).value).toBe('3m')
    expect(wrapper.find('.stub-EquityCandleChart').text()).toContain('3m')
  })

  it('订单明细 tab 展示回测产生的订单', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({
      orders: [
        liveOrder({ ord_id: 'bt-1', action: 'open_position', status: 'filled' }),
        liveOrder({ id: 2, ord_id: 'bt-2', action: 'close_position', status: 'filled' }),
      ],
    })
    await nextTick()

    const orderTab = wrapper.findAll('.vb-chart-tabs .vb-tab').find(tab => tab.text().includes('订单明细'))
    expect(orderTab?.text()).toContain('2')
    await orderTab?.trigger('click')
    await nextTick()

    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('2 orders')
    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('charts:false')
    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('table:true')
  })

  it('底部订单分布 tab 只展示订单图表', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({
      orders: [
        liveOrder({ ord_id: 'bt-1', action: 'open_position', status: 'filled' }),
        liveOrder({ id: 2, ord_id: 'bt-2', action: 'close_position', status: 'filled' }),
      ],
    })
    await nextTick()

    const distributionTab = wrapper.findAll('.vb-data-tabs .vb-tab').find(tab => tab.text().includes('订单分布'))
    expect(distributionTab?.text()).toContain('2')
    await distributionTab?.trigger('click')
    await nextTick()

    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('2 orders')
    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('charts:true')
    expect(wrapper.find('.stub-LiveOrderTable').text()).toContain('table:false')
  })

  it('币种收益 tab 按各标的已实现收益聚合展示', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = backtestResult({
      initial_capital: 1000,
      trades: [
        trade({ symbol: 'BTC-USDT-SWAP', timestamp: 1, action: 'open', side: 'buy', pos_side: 'long', price: 100, value: 100, commission: 0.1 }),
        trade({ symbol: 'BTC-USDT-SWAP', timestamp: 2, action: 'close', side: 'sell', pos_side: 'long', price: 112, value: 112, pnl: 12, commission: 0.1 }),
        trade({ symbol: 'ETH-USDT-SWAP', timestamp: 3, action: 'open', side: 'sell', pos_side: 'short', price: 50, value: 50, commission: 0.05 }),
        trade({ symbol: 'ETH-USDT-SWAP', timestamp: 4, action: 'close', side: 'buy', pos_side: 'short', price: 55, value: 55, pnl: -5, commission: 0.05 }),
      ],
    })
    await nextTick()

    const symbolTab = wrapper.findAll('.vb-chart-tabs .vb-tab').find(tab => tab.text().includes('币种收益'))
    expect(symbolTab?.text()).toContain('2')
    await symbolTab?.trigger('click')
    await nextTick()

    expect(wrapper.find('.sp-title').text()).toContain('币种收益 (2)')
    const rows = wrapper.findAll('.symbol-performance tbody tr')
    expect(rows).toHaveLength(2)
    expect(rows[0].text()).toContain('BTC')
    expect(rows[0].text()).toContain('12.00')
    expect(rows[0].text()).toContain('1.20%')
    expect(rows[1].text()).toContain('ETH')
    expect(rows[1].text()).toContain('-5.00')
  })

  it('页面只保留一个运行入口并在运行中禁用', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.activeResult = null
    await nextTick()
    const runResult = deferred<BacktestResult>()
    runBacktestMock.mockReturnValue(runResult.promise)

    const buttons = wrapper.findAll('button.run-btn')
    expect(buttons).toHaveLength(1)
    expect(buttons[0].text()).toBe('运行策略')

    await buttons[0].trigger('click')
    await nextTick()

    expect(wrapper.find('.param-modal').exists()).toBe(true)
    expect(runBacktestMock).not.toHaveBeenCalled()

    await wrapper.get('.param-submit-btn').trigger('click')
    await nextTick()

    const runningButtons = wrapper.findAll('button.run-btn')
    expect(runningButtons[0].attributes('disabled')).toBeDefined()
    expect(runningButtons[0].text()).toBe('运行中...')

    runResult.resolve(backtestResult({
      initial_capital: 1000,
      final_equity: 1000,
      equity_curve: [{ time: 1000, equity: 1000 }],
    }))
    await flushPromises()
  })

  it('运行策略时显示后端回测进度', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const runResult = deferred<BacktestResult>()
    runBacktestMock.mockReturnValue(runResult.promise)
    fetchBacktestProgressMock.mockResolvedValue(backtestProgress({
      stage: 'strategy',
      message: '执行策略 42/100 根K线',
      progress: 42,
      processed_candles: 42,
      total_candles: 100,
      strategy_progress: {
        step: 'strategy_evaluate_done',
        instrument_rules_source: 'simulated',
        evaluated_steps: 12,
        warmup_skipped: 30,
        actions: 3,
        intents: 1,
        risk_actions: 1,
        skipped_actions: 2,
        primary_context_candles: 160,
        risk_actions_total: 4,
      },
    }))

    await wrapper.find('button.run-btn').trigger('click')
    await nextTick()
    await wrapper.get('.param-submit-btn').trigger('click')
    await flushPromises()

    expect(wrapper.find('.vb-run-progress').exists()).toBe(true)
    expect(wrapper.find('.vb-run-progress').text()).toContain('执行策略')
    expect(wrapper.find('.vb-run-progress').text()).toContain('42%')
    expect(wrapper.find('.vb-run-progress').text()).toContain('42/100 K线')
    expect(wrapper.find('.vb-run-progress').text()).toContain('规格来源')
    expect(wrapper.find('.vb-run-progress').text()).toContain('模拟规格')
    expect(wrapper.find('.vb-run-progress').text()).toContain('已评估')
    expect(wrapper.find('.vb-run-progress').text()).toContain('12')
    expect(wrapper.find('.vb-run-progress').text()).toContain('执行意图')
    expect(wrapper.find('.vb-run-progress').text()).toContain('1')
    expect(wrapper.find('.vb-run-progress').text()).toContain('保护单动作')
    expect(wrapper.find('.vb-run-progress').text()).toContain('累计保护单动作')

    runResult.resolve(backtestResult({
      initial_capital: 1000,
      final_equity: 1000,
      equity_curve: [{ time: 1000, equity: 1000 }],
    }))
    await flushPromises()
  })

  it('运行前可以同时修改策略参数和引擎参数再开始回测', async () => {
    fetchStrategiesMock.mockResolvedValueOnce([
      strategyMeta({
        id: 'ml_trade_selector_forward_candidate_v1',
        name: 'ML Trade Selector Forward Candidate V1',
        runtime: {
          symbol: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          initial_capital: 1000,
          position_size: 0.2,
          stop_loss: 0.05,
          take_profit: 0.1,
          params: {
            leverage: 3,
            funding_rate_8h: 0.0001,
            strict_context_gating: false,
          },
        },
      }),
    ])
    const wrapper = mountBacktestView()
    await flushPromises()
    runBacktestMock.mockResolvedValue(backtestResult({
      strategy_id: 'ml_trade_selector_forward_candidate_v1',
      params: {
        leverage: 5,
        strict_context_gating: true,
      },
    }))

    await wrapper.get('button.run-btn').trigger('click')
    await nextTick()

    expect(wrapper.find('.param-modal').exists()).toBe(true)
    expect(wrapper.text()).toContain('策略参数')
    expect(wrapper.text()).toContain('回测引擎参数')

    await wrapper.get('.run-start-date-input').setValue('2026-05-01')
    await wrapper.get('.run-end-date-input').setValue('2026-05-10')
    await wrapper
      .get('.run-initial-capital-input')
      .setValue('2500')
    await wrapper
      .get('.param-editor-row[data-param-key="strict_context_gating"] select')
      .setValue('true')
    await wrapper
      .get('.param-editor-row[data-param-key="leverage"] input')
      .setValue('5')
    await wrapper
      .get('.param-editor-row[data-param-key="funding_rate_8h"] input')
      .setValue('0.0003')
    await wrapper.get('.param-submit-btn').trigger('click')
    await flushPromises()

    expect(runBacktestMock).toHaveBeenCalledWith(
      'ml_trade_selector_forward_candidate_v1',
      expect.objectContaining({
        start_date: '2026-05-01',
        end_date: '2026-05-10',
        initial_capital: 2500,
        params: expect.objectContaining({
          contract_mode: true,
          funding_rate_8h: 0.0003,
          leverage: 5,
          position_size: 0.2,
          strict_context_gating: true,
        }),
        progress_id: expect.any(String),
      }),
    )
  })

  it('运行前日期范围倒置时不发起回测', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()

    await wrapper.get('button.run-btn').trigger('click')
    await nextTick()

    await wrapper.get('.run-start-date-input').setValue('2026-05-10')
    await wrapper.get('.run-end-date-input').setValue('2026-05-01')
    await wrapper.get('.param-submit-btn').trigger('click')
    await nextTick()

    expect(runBacktestMock).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('结束日期必须晚于开始日期')
  })

  it('运行前初始资金非法时不发起回测', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()

    await wrapper.get('button.run-btn').trigger('click')
    await nextTick()

    await wrapper.get('.run-initial-capital-input').setValue('0')
    await wrapper.get('.param-submit-btn').trigger('click')
    await nextTick()

    expect(runBacktestMock).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('初始资金必须大于 0')
  })

  it('历史回测删除按钮只删除记录，不触发详情选择', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    deleteBacktestResultMock.mockResolvedValue({})
    const store = useBacktestStore()
    store.history = [
      backtestResult({ result_id: 'remove', strategy_name: 'Remove Me' }),
      backtestResult({ result_id: 'keep', strategy_name: 'Keep Me' }),
    ]
    store.activeResult = backtestResult({ result_id: 'keep', strategy_name: 'Keep Me' })
    fetchBacktestDetailMock.mockClear()
    await nextTick()

    await wrapper.find('.hb-delete').trigger('click')
    await flushPromises()

    expect(wrapper.find('.vb-history-body').exists()).toBe(true)
    expect(deleteBacktestResultMock).toHaveBeenCalledTimes(1)
    expect(deleteBacktestResultMock).toHaveBeenCalledWith('remove')
    expect(fetchBacktestDetailMock).not.toHaveBeenCalled()
    expect(store.history.map(item => item.result_id)).toEqual(['keep'])
    expect(wrapper.text()).not.toContain('Remove Me')
    expect(wrapper.text()).toContain('Keep Me')
  })

  it('历史回测列表显示结果完整性标记', async () => {
    const wrapper = mountBacktestView()
    await flushPromises()
    const store = useBacktestStore()
    store.history = [
      backtestResult({
        result_id: 'legacy',
        strategy_name: 'Legacy Runtime',
        backtest_result_integrity: {
          status: 'unverified',
          label: '旧结果待复核',
          issues: ['runtime_action_summary_missing'],
        },
      }),
      backtestResult({
        result_id: 'invalid',
        strategy_name: 'Invalid Runtime',
        backtest_result_integrity: {
          status: 'invalid',
          label: '结果不可信',
          issues: ['planned_exit_missing'],
        },
      }),
      backtestResult({ result_id: 'ok', strategy_name: 'Clean Result' }),
    ]
    await nextTick()

    expect(wrapper.find('.vb-history-body').exists()).toBe(true)
    const rows = wrapper.findAll('.vb-history-item')

    expect(rows[0]?.find('.hb-integrity').text()).toBe('旧')
    expect(rows[0]?.find('.hb-integrity').classes()).toContain('unverified')
    expect(rows[1]?.find('.hb-integrity').text()).toBe('异常')
    expect(rows[1]?.find('.hb-integrity').classes()).toContain('invalid')
    expect(rows[2]?.find('.hb-integrity').exists()).toBe(false)
  })
})

function mountBacktestView() {
  return mount(BacktestView, {
    global: {
      plugins: [createPinia()],
      stubs: {
        Teleport: true,
        StrategySelector: stubComponent('StrategySelector'),
        BacktestResultCard: backtestResultCardStub(),
        EquityCandleChart: equityCandleChartStub(),
        LiveOrderTable: liveOrderTableStub(),
        ThemeSelect: themeSelectStub(),
        ThemeDateInput: themeDateInputStub(),
      },
    },
  })
}

function themeSelectStub(): Component {
  return defineComponent({
    name: 'ThemeSelect',
    props: {
      modelValue: { type: String, default: '' },
      options: { type: Array, default: () => [] },
      placeholder: { type: String, default: '请选择' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit }) {
      return () => h('select', {
        class: 'theme-select-stub',
        value: props.modelValue,
        onChange: (event: Event) => emit('update:modelValue', (event.target as HTMLSelectElement).value),
      }, [
        h('option', { value: '' }, props.placeholder),
        ...(props.options as Array<{ value: string; label: string }>).map(option =>
          h('option', { value: option.value }, option.label)
        ),
      ])
    },
  })
}

function themeDateInputStub(): Component {
  return defineComponent({
    name: 'ThemeDateInput',
    props: {
      modelValue: { type: String, default: '' },
      placeholder: { type: String, default: '选择日期' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
      return () => h('input', {
        ...attrs,
        class: ['theme-date-input-stub', attrs.class],
        type: 'date',
        value: props.modelValue,
        placeholder: props.placeholder,
        onInput: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).value),
        onChange: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).value),
      })
    },
  })
}

function backtestResultCardStub(): Component {
  return defineComponent({
    name: 'BacktestResultCard',
    props: {
      result: { type: Object, required: true },
      running: { type: Boolean, default: false },
    },
    setup(props) {
      return () => h(
        'div',
        { class: 'stub-BacktestResultCard' },
        `${(props.result as BacktestResult).strategy_name || ''} ${props.running ? 'running' : ''}`,
      )
    },
  })
}

function equityCandleChartStub(): Component {
  return defineComponent({
    name: 'EquityCandleChart',
    props: {
      candles: { type: Array, default: () => [] },
      snapshots: { type: Array, default: () => [] },
      timeframe: { type: String, default: '' },
      trades: { type: Array, default: () => [] },
    },
    setup(props) {
      return () => h(
        'div',
        { class: 'stub-EquityCandleChart' },
        `${props.timeframe} ${props.candles.length} candles ${props.snapshots.length} snapshots ${props.trades.length} trades`,
      )
    },
  })
}

function liveOrderTableStub(): Component {
  return defineComponent({
    name: 'LiveOrderTable',
    props: {
      orders: { type: Array, default: () => [] },
      showCharts: { type: Boolean, default: true },
      showTable: { type: Boolean, default: true },
    },
    setup(props) {
      return () => h(
        'div',
        { class: 'stub-LiveOrderTable' },
        `${props.orders.length} orders charts:${props.showCharts} table:${props.showTable}`,
      )
    },
  })
}

function stubComponent(name: string): Component {
  return defineComponent({
    name,
    setup(_, { slots }) {
      return () => h('div', { class: `stub-${name}` }, slots.default?.())
    },
  })
}

function liveOrder(overrides: Partial<LiveOrder> = {}): LiveOrder {
  return {
    id: 1,
    ord_id: 'bt-1',
    client_order_id: 'bt-cl-1',
    parent_order_id: '',
    parent_client_order_id: '',
    actual_order_id: '',
    actual_client_order_id: '',
    inst_id: 'BTC-USDT-SWAP',
    symbol: 'BTC-USDT-SWAP',
    order_type: 'market',
    side: 'buy',
    sz: 1,
    px: null,
    fill_count: 1,
    filled_size: 1,
    filled_quantity: 1,
    avg_fill_price: 100,
    fill_notional: 100,
    remaining_size: 0,
    total_fee: 0,
    fee_ccy: null,
    first_fill_ts: 1,
    last_fill_ts: 1,
    fill_source: 'historical_live_backtest',
    action: 'open_position',
    success: true,
    status: 'filled',
    error_message: '',
    mode: 'simulated',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    run_id: 'result',
    timestamp: 1,
    arrival_ts: null,
    arrival_mid_px: null,
    arrival_bid_px: null,
    arrival_ask_px: null,
    created_at: 1,
    ...overrides,
    reference_price: overrides.reference_price ?? null,
    reference_price_source: overrides.reference_price_source ?? '',
    reference_price_missing: overrides.reference_price_missing ?? false,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
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
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
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
    total_trades: 1,
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

function backtestProgress(overrides: Partial<BacktestProgress> = {}): BacktestProgress {
  return {
    run_id: 'progress',
    strategy_id: 'multi_timeframe_dual_v12',
    status: 'running',
    stage: 'strategy',
    message: '执行策略',
    progress: 40,
    processed_candles: 10,
    total_candles: 25,
    started_at: '',
    updated_at: '',
    ...overrides,
  }
}

function trade(overrides: Partial<BacktestTrade>): BacktestTrade {
  return {
    timestamp: 0,
    datetime: '',
    entry_time: '',
    exit_time: '',
    side: 'buy',
    action: '',
    pos_side: '',
    price: 0,
    entry_price: 0,
    exit_price: 0,
    quantity: 1,
    value: 0,
    commission: 0,
    pnl: 0,
    pnl_pct: 0,
    funding: 0,
    equity: 0,
    reason: '',
    ...overrides,
  }
}

function snapshot(overrides: Partial<BacktestEquitySnapshot>): BacktestEquitySnapshot {
  return {
    time: 0,
    equity: 0,
    cash: 0,
    position_value: 0,
    position_notional: 0,
    unrealized_pnl: 0,
    position_side: 'flat',
    leverage: 1,
    ...overrides,
  }
}
