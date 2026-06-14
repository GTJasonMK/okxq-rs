import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import LivePositionPanel from '@/components/live/LivePositionPanel.vue'
import type { LiveOrder, Position } from '@/types'
import { liveOrder } from './fixtures/liveStrategy'

const HISTORY_ROW_HEIGHT = 40

describe('LivePositionPanel', () => {
  it('展示 OKX 当前持仓并从策略记录提炼历史仓位', () => {
    const wrapper = mount(LivePositionPanel, {
      props: {
        mode: 'simulated',
        positions: [
          position({ inst_id: 'BTC-USDT-SWAP', pos_side: 'long', pos: 2, upl: 12.5, upl_ratio: 0.025 }),
          position({ inst_id: 'ETH-USDT-SWAP', pos_side: 'short', pos: -1, upl: -3, upl_ratio: -0.01 }),
          position({ inst_id: 'DOGE-USDT-SWAP', pos_side: 'long', pos: 0 }),
        ],
        historyOrders: [
          liveOrder({
            id: 1,
            inst_id: 'BTC-USDT-SWAP',
            side: 'buy',
            action: 'open_position',
            status: 'filled',
            timestamp: 1_780_000_000_000,
            px: 100,
            sz: 2,
          }),
          liveOrder({
            id: 2,
            inst_id: 'BTC-USDT-SWAP',
            side: 'sell',
            action: 'close_position',
            status: 'filled',
            timestamp: 1_780_000_060_000,
            px: 105,
            sz: 2,
          }),
          liveOrder({
            id: 3,
            inst_id: 'ETH-USDT-SWAP',
            side: 'sell',
            action: 'place_risk_order',
            success: false,
            status: 'submit_failed',
            timestamp: 1_780_000_120_000,
          }),
          liveOrder({
            id: 4,
            inst_id: 'SOL-USDT-SWAP',
            side: 'sell',
            action: 'open_position',
            success: false,
            status: 'risk_blocked',
            timestamp: 1_780_000_180_000,
          }),
        ],
      },
    })

    expect(wrapper.find('.lp-title').text()).toBe('仓位')
    expect(wrapper.find('.lp-subtitle').text()).toContain('模拟盘')
    expect(wrapper.find('.lp-subtitle').text()).toContain('OKX 当前持仓 2')
    expect(wrapper.find('.lp-subtitle').text()).toContain('历史 2')

    const currentRows = wrapper.findAll('.lp-section').at(0)?.findAll('tbody tr') ?? []
    expect(currentRows).toHaveLength(2)
    expect(currentRows[0]?.text()).toContain('BTC-USDT-SWAP')
    expect(currentRows[0]?.text()).toContain('多')
    expect(currentRows[0]?.text()).toContain('12.50')
    expect(currentRows[1]?.text()).toContain('ETH-USDT-SWAP')
    expect(currentRows[1]?.text()).toContain('空')

    const historyRows = wrapper.findAll('.lp-section').at(1)?.findAll('tbody tr') ?? []
    expect(historyRows).toHaveLength(2)
    expect(historyRows[0]?.text()).toContain('平多')
    expect(historyRows[0]?.text()).toContain('平仓')
    expect(historyRows[1]?.text()).toContain('开多')
    expect(wrapper.text()).not.toContain('保护单')
    expect(wrapper.text()).not.toContain('风控拦截')
  })

  it('无仓位和无历史时展示空状态', () => {
    const wrapper = mount(LivePositionPanel, {
      props: {
        mode: 'live',
        positions: [],
        historyOrders: [],
      },
    })

    const emptyStates = wrapper.findAll('.empty-state')
    expect(emptyStates).toHaveLength(2)
    expect(emptyStates[0]?.text()).toContain('当前 OKX 账户暂无合约持仓')
    expect(emptyStates[1]?.text()).toContain('当前范围暂无历史仓位记录')
    expect(wrapper.find('.lp-subtitle').text()).toContain('实盘')
  })

  it('待确认的开仓和平仓请求进入历史仓位但保护单不进入', () => {
    const wrapper = mount(LivePositionPanel, {
      props: {
        mode: 'live',
        positions: [],
        historyOrders: [
          liveOrder({
            id: 1,
            inst_id: 'BTC-USDT-SWAP',
            side: 'buy',
            action: 'open_position',
            status: 'submit_unknown',
            success: false,
            timestamp: 1_780_000_000_000,
            error_message: '提交 OKX 订单后响应结果待确认',
          }),
          liveOrder({
            id: 2,
            inst_id: 'BTC-USDT-SWAP',
            side: 'sell',
            action: 'close_position',
            status: 'submit_unknown',
            success: false,
            timestamp: 1_780_000_060_000,
            error_message: '提交 OKX 平仓单后响应结果待确认',
          }),
          liveOrder({
            id: 3,
            inst_id: 'BTC-USDT-SWAP',
            side: 'sell',
            action: 'place_risk_order',
            status: 'algo_submit_unknown',
            success: false,
            timestamp: 1_780_000_120_000,
          }),
        ],
      },
    })

    expect(wrapper.find('.lp-subtitle').text()).toContain('历史 2')
    const historyRows = wrapper.findAll('.history-wrap tbody tr')
    expect(historyRows).toHaveLength(2)
    expect(historyRows[0]?.text()).toContain('平多')
    expect(historyRows[0]?.text()).toContain('提交结果待确认')
    expect(historyRows[1]?.text()).toContain('开多')
    expect(historyRows[1]?.text()).toContain('提交结果待确认')
    expect(wrapper.text()).not.toContain('保护单')
  })

  it('大历史仓位列表只挂载视窗附近行，避免运行页轮询后全量 DOM 重渲染', () => {
    const historyOrders = perfHistoryOrders(300)

    const wrapper = mount(LivePositionPanel, {
      props: {
        mode: 'simulated',
        positions: [],
        historyOrders,
      },
    })

    expect(wrapper.findAll('.history-wrap tbody tr').length).toBeLessThanOrEqual(24)
    expect(wrapper.text()).toContain('PERF-299-USDT-SWAP')

    wrapper.unmount()
  })

  it('历史仓位滚动时切换可见窗口并保持挂载行数有界', async () => {
    const historyOrders = perfHistoryOrders(300)
    const wrapper = mount(LivePositionPanel, {
      props: {
        mode: 'simulated',
        positions: [],
        historyOrders,
      },
    })
    const viewport = wrapper.find('.history-wrap')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: HISTORY_ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = HISTORY_ROW_HEIGHT * 120
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('PERF-179-USDT-SWAP')
    expect(wrapper.findAll('.history-wrap tbody tr').length).toBeLessThanOrEqual(22)

    wrapper.unmount()
  })
})

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

function perfHistoryOrders(count: number): LiveOrder[] {
  const base = 1_780_000_000_000
  return Array.from({ length: count }, (_, index) => {
    const side: 'buy' | 'sell' = index % 2 === 0 ? 'buy' : 'sell'
    const action = index % 5 === 0 ? 'close_position' : 'open_position'
    return liveOrder({
      id: index + 1,
      ord_id: `perf-order-${index}`,
      inst_id: `PERF-${String(index).padStart(3, '0')}-USDT-SWAP`,
      symbol: `PERF-${String(index).padStart(3, '0')}-USDT-SWAP`,
      side,
      action,
      status: 'filled',
      success: true,
      timestamp: base + index * 1000,
      created_at: base + index * 1000,
      px: 100 + index,
      sz: 1 + index / 100,
    })
  })
}
