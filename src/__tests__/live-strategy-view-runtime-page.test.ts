import { flushPromises } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import type { VueWrapper } from '@vue/test-utils'
import type { Position } from '@/types'
import {
  equityHistory,
  equitySnapshot,
  liveExecutionPlan,
  liveOrder,
  status,
} from './fixtures/liveStrategy'
import {
  fetchAvailableStrategiesMock,
  fetchLiveEquityMock,
  fetchLiveExecutionPlansMock,
  fetchLiveOrdersMock,
  fetchPositionsMock,
  fetchLiveStatusMock,
  mountLiveStrategyPage,
  setupLiveStrategyViewHarness,
  startLiveStrategyMock,
  type LiveStrategyTestPinia,
} from './helpers/liveStrategyViewHarness'

vi.mock('@/components/strategy/EquityCandleChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'EquityCandleChartStub',
      props: {
        candles: { type: Array, default: () => [] },
        snapshots: { type: Array, default: () => [] },
        trades: { type: Array, default: () => [] },
        timeframe: { type: String, default: '' },
        title: { type: String, default: '' },
      },
      setup(props) {
        return () => h('div', {
          class: 'equity-candle-chart-stub',
          'data-candles': props.candles.length,
          'data-snapshots': props.snapshots.length,
          'data-trades': props.trades.length,
          'data-timeframe': props.timeframe,
        }, props.title)
      },
    }),
  }
})

describe('LiveStrategyView runtime page', () => {
  let pinia!: LiveStrategyTestPinia
  setupLiveStrategyViewHarness((value) => {
    pinia = value
  })

  it('renders live equity candles above tabbed runtime data panels', async () => {
    const base = 1_780_000_000_000
    fetchLiveStatusMock.mockResolvedValue(status({
      running: true,
      status: 'running',
      run_id: 'run-live',
      timeframe: '15m',
    }))
    fetchLiveEquityMock.mockResolvedValue(equityHistory({
      run_id: 'run-live',
      count: 2,
      snapshots: [
        equitySnapshot({
          run_id: 'run-live',
          timestamp: base,
          equity: 1000,
        }),
        equitySnapshot({
          run_id: 'run-live',
          timestamp: base + 15 * 60_000,
          equity: 1012,
          position_side: 'long',
          price: 101,
          entry_price: 100,
          quantity: 2,
          unrealized_pnl: 2,
        }),
      ],
    }))
    fetchPositionsMock.mockResolvedValue([
      position({
        inst_id: 'BTC-USDT-SWAP',
        pos_side: 'long',
        pos: 2,
        avg_px: 100,
        mark_px: 101,
        upl: 2,
      }),
    ])
    fetchLiveOrdersMock.mockResolvedValue([
      liveOrder({
        run_id: 'run-live',
        timestamp: base + 15 * 60_000,
        side: 'buy',
        action: 'open_position',
        px: 100,
        sz: 2,
      }),
    ])
    fetchLiveExecutionPlansMock.mockResolvedValue([
      liveExecutionPlan({
        mode: 'simulated',
        entry_run_id: 'run-live',
        planned_exit_time: Date.now() + 30 * 60_000,
      }),
    ])

    const wrapper = mountLiveStrategyPage(pinia)
    await flushPromises()

    expect(wrapper.find('.vl-sidebar .vl-control-panel').exists()).toBe(true)
    expect(wrapper.find('.vl-sidebar .vl-run-summary-panel').exists()).toBe(false)
    expect(wrapper.find('.vl-sidebar > .vl-execution-log-panel').exists()).toBe(true)
    expect(wrapper.find('.vl-main > .vl-run-summary-panel').exists()).toBe(true)
    expect(wrapper.findAll('.vl-main > .vl-run-summary-panel .vl-kpi-card').length).toBeGreaterThan(0)
    expect(wrapper.find('.vl-main > .vl-execution-log-panel').exists()).toBe(false)
    expect(wrapper.find('.vl-main > .vl-bottom-panels').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-workbench').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-focus-panel').exists()).toBe(true)
    expect(wrapper.findAll('.vl-focus-tabs > .vl-tab').map(tab => tab.text())).toEqual([
      '余额K线',
      '决策',
      '权益明细 2',
      '仓位 1',
      '退出计划 1',
    ])
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-decision-card').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-equity-bucket').exists()).toBe(true)
    expect(wrapper.find('.vl-main .vl-data-head').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-data-tabs').exists()).toBe(false)
    expect(wrapper.find('.vl-subnav').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('支持品种')
    expect(wrapper.text()).not.toContain('支持周期')
    expect(wrapper.text()).not.toContain('运行结构')

    const html = wrapper.html()
    const controlPanelIndex = html.indexOf('vl-control-panel')
    const chartPanelIndex = html.indexOf('vl-chart-panel')
    const summaryPanelIndex = html.indexOf('vl-run-summary-panel')
    expect(controlPanelIndex).toBeGreaterThanOrEqual(0)
    expect(chartPanelIndex).toBeGreaterThanOrEqual(0)
    expect(summaryPanelIndex).toBeGreaterThan(chartPanelIndex)

    const chart = wrapper.find('.equity-candle-chart-stub')
    expect(chart.exists()).toBe(true)
    expect(chart.attributes('data-candles')).toBe('2')
    expect(chart.attributes('data-snapshots')).toBe('2')
    expect(chart.attributes('data-trades')).toBe('1')
    expect(chart.attributes('data-timeframe')).toBe('15m')

    await clickFocusTab(wrapper, '决策')
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-decision-card').exists()).toBe(true)
    expect(wrapper.find('.vl-main .vl-focus-panel .equity-candle-chart-stub').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-equity-bucket').exists()).toBe(false)
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-trigger-select').exists()).toBe(true)

    await clickFocusTab(wrapper, '余额K线')
    expect(wrapper.find('.vl-main .vl-focus-panel .equity-candle-chart-stub').exists()).toBe(true)

    await clickFocusTab(wrapper, '权益明细')
    expect(wrapper.find('.le-panel').exists()).toBe(true)
    expect(wrapper.find('.vl-main .vl-focus-panel .equity-candle-chart-stub').exists()).toBe(false)

    await clickFocusTab(wrapper, '仓位')
    expect(wrapper.find('.lp-panel').exists()).toBe(true)
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-trigger-select').exists()).toBe(false)
    expect(wrapper.text()).toContain('当前持仓')
    expect(wrapper.text()).toContain('历史仓位')

    await clickFocusTab(wrapper, '退出计划')
    expect(wrapper.find('.lep-panel').exists()).toBe(true)
    expect(wrapper.find('.vl-main .vl-focus-panel .vl-trigger-select').exists()).toBe(false)
    expect(wrapper.text()).toContain('等待退出')
  })

  it('启动前弹出参数选择并用用户参数启动策略', async () => {
    fetchAvailableStrategiesMock.mockResolvedValue([
      {
        id: 'runtime_candidate_breakout_v1',
        name: 'Runtime Candidate Breakout V1',
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
          params: {
            leverage: 3,
            require_stop_loss: true,
            entry_threshold: 1.5,
          },
        },
        visualization: {},
        decision_contract: {},
      },
    ])
    fetchLiveStatusMock.mockResolvedValue(status({ running: false, status: 'stopped' }))

    const wrapper = mountLiveStrategyPage(pinia)
    await flushPromises()

    const capitalInput = wrapper.find<HTMLInputElement>('input[name="live-initial-capital"]')
    expect(capitalInput.exists()).toBe(true)
    await capitalInput.setValue('100')

    const startButton = wrapper.find('.btn.start')
    expect(startButton.attributes('disabled')).toBeUndefined()
    await startButton.trigger('click')
    await flushPromises()

    expect(document.body.querySelector('.param-modal')).toBeTruthy()
    const initialCapitalInput = document.body.querySelector<HTMLInputElement>('[name="secondary-param-initial_capital"]')
    expect(initialCapitalInput).toBeTruthy()
    expect(initialCapitalInput!.value).toBe('100')
    const leverageInput = document.body.querySelector<HTMLInputElement>('[name="secondary-param-leverage"]')
    expect(leverageInput).toBeTruthy()
    leverageInput!.value = '5'
    leverageInput!.dispatchEvent(new Event('input', { bubbles: true }))
    const thresholdInput = document.body.querySelector<HTMLInputElement>('[name="param-entry_threshold"]')
    expect(thresholdInput).toBeTruthy()
    thresholdInput!.value = '2.25'
    thresholdInput!.dispatchEvent(new Event('input', { bubbles: true }))
    document.body.querySelector<HTMLButtonElement>('.param-submit-btn')?.click()
    await flushPromises()

    expect(startLiveStrategyMock).toHaveBeenCalledWith(expect.objectContaining({
      strategy_id: 'runtime_candidate_breakout_v1',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      risk_timeframe: '1m',
      initial_capital: 100,
      position_size: 0.25,
      stop_loss: 0,
      take_profit: 0,
      check_interval: 60,
      mode: 'simulated',
      params: expect.objectContaining({
        leverage: 5,
        require_stop_loss: true,
        entry_threshold: 2.25,
      }),
    }))
  })
})

async function clickFocusTab(wrapper: VueWrapper, text: string) {
  const tab = wrapper.findAll('.vl-focus-tabs > .vl-tab')
    .find(item => item.text().includes(text))
  expect(tab, `missing focus tab: ${text}`).toBeTruthy()
  await tab!.trigger('click')
  await flushPromises()
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
