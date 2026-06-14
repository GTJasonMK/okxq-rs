import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import LiveOrderTable from '@/components/live/LiveOrderTable.vue'
import type { LiveOrder } from '@/types'
import { formatPrice } from '@/utils/format'

const ROW_HEIGHT = 34

describe('LiveOrderTable', () => {
  it('空订单状态说明为什么空以及下一步', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [],
      },
    })

    expect(wrapper.find('.empty-state strong').text()).toBe('当前范围暂无策略订单')
    expect(wrapper.find('.empty-state').text()).toContain('策略生成入场、平仓、挂单或风控拦截动作后')
    expect(wrapper.find('.empty-state').text()).toContain('查看决策页')
    expect(wrapper.find('.empty-state').text()).toContain('当前 run')
  })

  it('缺失动作时间和创建时间时显示占位符', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({ timestamp: null, created_at: 0 })],
      },
    })

    expect(wrapper.find('.time-cell').text()).toBe('--')
  })

  it('未知订单价格和数量显示占位符而不是 0', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({ px: null, sz: null })],
      },
    })

    const numericCells = wrapper.findAll('tbody tr:first-child .num')
    expect(numericCells[0]?.text()).toBe('--')
    expect(numericCells[1]?.text()).toBe('--')
  })

  it('缺少成交和委托价时显示回测参考价来源', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({
          px: null,
          avg_fill_price: null,
          reference_price: 100,
          reference_price_source: 'entry_price_fallback',
          reference_price_missing: true,
        })],
      },
    })

    const priceCell = wrapper.find('tbody tr:first-child .price-cell')
    expect(priceCell.text()).toContain(formatPrice(100))
    expect(priceCell.text()).toContain('参考价 entry fallback')
  })

  it('有真实成交时优先显示成交均价和成交数量', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({
          px: 100,
          sz: 3,
          fill_count: 2,
          avg_fill_price: 106.6666666667,
          filled_size: 2,
          filled_quantity: 2,
          remaining_size: 1,
          status: 'partially_filled',
        })],
      },
    })

    const numericCells = wrapper.findAll('tbody tr:first-child .num')
    expect(numericCells[0]?.text()).toBe(formatPrice(106.6666666667))
    expect(numericCells[1]?.text()).toBe('2.0000 / 3.0000')
  })

  it('可以只显示订单表格并隐藏分布图', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order()],
        showCharts: false,
      },
    })

    expect(wrapper.find('.lo-charts').exists()).toBe(false)
    expect(wrapper.find('.lo-wrap table').exists()).toBe(true)
  })

  it('可以只显示订单分布图并隐藏表格', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order()],
        showTable: false,
      },
    })

    expect(wrapper.find('.lo-charts').exists()).toBe(true)
    expect(wrapper.find('.lo-wrap').exists()).toBe(false)
    expect(wrapper.find('table').exists()).toBe(false)
  })

  it('动作时间缺失时回退显示创建时间', () => {
    const createdAt = Date.parse('2026-05-28T00:01:00.000Z')
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({ timestamp: null, created_at: createdAt })],
      },
    })

    expect(wrapper.find('.time-cell').text()).toBe(
      formatOrderTime(createdAt),
    )
  })

  it('无效动作时间回退创建时间并参与最新优先排序', () => {
    const fallbackCreatedAt = Date.parse('2026-05-28T00:01:00.000Z')
    const latestTimestamp = Date.parse('2026-05-28T01:00:00.000Z')
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            ord_id: 'fallback_order',
            timestamp: Number.POSITIVE_INFINITY,
            created_at: fallbackCreatedAt,
          }),
          order({
            id: 2,
            ord_id: 'latest_order',
            timestamp: latestTimestamp,
            created_at: latestTimestamp,
          }),
        ],
      },
    })

    const rows = wrapper.findAll('tbody tr')
    expect(rows[0]?.find('.id-cell').text()).toBe('latest_order')
    expect(rows[1]?.find('.id-cell').text()).toBe('fallback_ord')
    expect(rows[1]?.find('.time-cell').text()).toBe(formatOrderTime(fallbackCreatedAt))
  })

  it('订单时间显示日期避免跨日记录误判', () => {
    const createdAt = Date.parse('2026-05-28T16:01:02.000Z')
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [order({ timestamp: createdAt, created_at: 0 })],
      },
    })

    expect(wrapper.find('.time-cell').text()).toBe(formatOrderTime(createdAt))
    expect(wrapper.find('.time-cell').text()).toContain('05/29')
  })

  it('长订单列表保持在滚动容器内渲染', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: Array.from({ length: 30 }, (_, index) => order({ id: index + 1 })),
      },
    })

    expect(wrapper.find('.lo-wrap table').exists()).toBe(true)
    expect(wrapper.findAll('tbody tr')).toHaveLength(30)
  })

  it('运行页订单表只挂载视窗附近行，避免 5 秒刷新时全量 DOM 重渲染', () => {
    const orders = perfOrders(300)

    const wrapper = mount(LiveOrderTable, {
      props: { orders },
    })

    expect(wrapper.findAll('tbody tr').length).toBeLessThanOrEqual(32)
    expect(wrapper.find('tbody tr .id-cell').text()).toBe('order_0300')

    wrapper.unmount()
  })

  it('运行页订单表滚动时切换可见窗口并保持最新优先顺序', async () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: perfOrders(300),
      },
    })
    const viewport = wrapper.find('.lo-wrap')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = ROW_HEIGHT * 120
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('order_0180')
    expect(wrapper.findAll('.id-cell').length).toBeLessThanOrEqual(18)

    wrapper.unmount()
  })

  it('订单明细始终按最新动作时间优先展示', () => {
    const oldTimestamp = Date.parse('2026-05-28T00:00:00.000Z')
    const latestTimestamp = Date.parse('2026-05-28T01:00:00.000Z')
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({ id: 1, ord_id: 'old_order', timestamp: oldTimestamp, created_at: oldTimestamp }),
          order({ id: 2, ord_id: 'latest_order', timestamp: latestTimestamp, created_at: latestTimestamp }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('最新优先')
    expect(wrapper.find('tbody tr:first-child .id-cell').text()).toBe('latest_order')
    expect(wrapper.find('tbody tr:first-child .time-cell').text()).toBe(formatOrderTime(latestTimestamp))
  })

  it('合约订单直接显示开平仓动作而不是裸买卖', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({ id: 1, side: 'sell', status: 'filled', action: 'open_position', timestamp: 2 }),
          order({ id: 2, side: 'buy', status: 'filled', action: 'close_position', timestamp: 1 }),
        ],
      },
    })

    const actions = wrapper.findAll('.action-badge').map(item => item.text())
    expect(actions).toEqual(['开空', '平空'])
    expect(actions).not.toContain('卖')
    expect(wrapper.find('.lo-charts').exists()).toBe(true)
    expect(wrapper.findAll('.lo-mix-track i')).toHaveLength(2)
    expect(wrapper.findAll('.lo-timeline i')).toHaveLength(2)
  })

  it('close_position 不依赖 closed 状态也按平仓统计', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'buy',
            status: 'filled',
            action: 'close_position',
            timestamp: 1,
          }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('平仓 1')
    expect(wrapper.find('.lo-summary').text()).toContain('开仓 0')
    expect(wrapper.find('.action-badge').text()).toBe('平空')
    expect(wrapper.find('.action-cell').text()).toContain('平仓')
    expect(wrapper.find('.action-cell').text()).not.toContain('layer:')
  })

  it('交易所提交中的 close_position 订单按平仓显示', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'sell',
            status: 'submitted',
            action: 'close_position',
            timestamp: 1,
          }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('平仓 1')
    expect(wrapper.find('.lo-summary').text()).toContain('开仓 0')
    expect(wrapper.find('.action-badge').text()).toBe('平多')
    expect(wrapper.find('.action-cell').text()).toContain('平仓')
  })

  it('交易所已接受的撤单和改单请求不按开仓统计', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'hold',
            status: 'cancel_requested',
            success: true,
            action: 'cancel_order',
            timestamp: 2,
          }),
          order({
            id: 2,
            side: 'hold',
            status: 'modify_requested',
            success: true,
            action: 'modify_order',
            timestamp: 1,
          }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('开仓 0')
    expect(wrapper.find('.lo-summary').text()).toContain('平仓 0')
    expect(wrapper.findAll('.action-badge').map(item => item.text())).toEqual(['撤单', '改单'])
    expect(wrapper.findAll('.action-cell').map(item => item.text())).toEqual(['撤单', '改单'])
  })

  it('提交结果待确认订单不计入失败摘要', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'buy',
            status: 'submit_unknown',
            success: false,
            action: 'open_position',
            error_message: 'OKX 下单请求已发出，但响应结果待同步确认',
          }),
          order({
            id: 2,
            side: 'sell',
            status: 'algo_submit_unknown',
            success: false,
            action: 'place_risk_order',
            error_message: 'OKX 独立保护单提交结果待确认',
          }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('失败 0')
    expect(wrapper.findAll('.status').map(item => item.text())).toEqual([
      '保护单提交待确认',
      '提交结果待确认',
    ])
  })

  it('保护单动作显示为可读标签', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'sell',
            status: 'submit_failed',
            success: false,
            action: 'place_risk_order',
            error_message: '保护单缺少有效触发价',
          }),
        ],
      },
    })

    expect(wrapper.find('.action-cell').text()).toContain('保护单')
    expect(wrapper.find('.action-cell').text()).toContain('保护单缺少有效触发价')
    expect(wrapper.find('.status').text()).toBe('提交失败')
  })

  it('风险拦截使用规范动作摘要显示', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({
            id: 1,
            side: 'sell',
            status: 'risk_blocked',
            success: false,
            action: 'open_position',
          }),
        ],
      },
    })

    expect(wrapper.find('.lo-summary').text()).toContain('拦截 1')
    expect(wrapper.find('.lo-summary').text()).toContain('失败 1')
    expect(wrapper.find('.action-badge').text()).toBe('风控拦截')
    expect(wrapper.find('.action-cell').text()).toContain('开仓')
    expect(wrapper.find('.action-cell').text()).not.toContain('layer:')
  })

  it('现货订单保留买入卖出语义', () => {
    const wrapper = mount(LiveOrderTable, {
      props: {
        orders: [
          order({ id: 1, inst_id: 'BTC-USDT', side: 'buy', status: 'filled', action: 'open_position' }),
        ],
      },
    })

    expect(wrapper.find('.action-badge').text()).toBe('买入')
    expect(wrapper.find('.action-cell').text()).toContain('开仓')
  })
})

function order(overrides: Partial<LiveOrder> = {}): LiveOrder {
  return {
    id: 0,
    ord_id: '',
    client_order_id: '',
    parent_order_id: '',
    parent_client_order_id: '',
    actual_order_id: '',
    actual_client_order_id: '',
    inst_id: 'BTC-USDT-SWAP',
    symbol: 'BTC-USDT-SWAP',
    order_type: 'market',
    side: 'sell',
    sz: 1,
    px: 100,
    reference_price: null,
    reference_price_source: '',
    reference_price_missing: false,
    fill_count: 0,
    filled_size: null,
    filled_quantity: null,
    avg_fill_price: null,
    fill_notional: null,
    remaining_size: null,
    total_fee: null,
    fee_ccy: null,
    first_fill_ts: null,
    last_fill_ts: null,
    fill_source: '',
    action: 'open_position',
    success: true,
    status: 'filled',
    error_message: '',
    mode: 'simulated',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    run_id: 'run',
    timestamp: 0,
    arrival_ts: null,
    arrival_mid_px: null,
    arrival_bid_px: null,
    arrival_ask_px: null,
    created_at: 0,
    ...overrides,
  }
}

function perfOrders(count: number): LiveOrder[] {
  const base = Date.parse('2026-05-28T00:00:00.000Z')
  return Array.from({ length: count }, (_, index) => {
    const seq = index + 1
    return order({
      id: seq,
      ord_id: `order_${String(seq).padStart(4, '0')}`,
      client_order_id: `client_${seq}`,
      side: seq % 3 === 0 ? 'buy' : 'sell',
      sz: 0.001 + seq / 10_000,
      px: 60_000 + seq,
      action: seq % 7 === 0
        ? 'close_position'
        : seq % 11 === 0
          ? 'open_position'
          : 'open_position',
      success: seq % 11 !== 0,
      status: seq % 11 === 0 ? 'risk_blocked' : 'filled',
      error_message: seq % 11 === 0
        ? `risk blocked for order ${seq} with diagnostic details and guard state`
        : '',
      timestamp: base + seq * 1000,
      created_at: base + seq * 1000,
    })
  })
}

function formatOrderTime(timestamp: number): string {
  return new Date(timestamp).toLocaleString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}
