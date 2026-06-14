import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import EquityCandleTooltip from '@/components/strategy/EquityCandleTooltip.vue'
import type { HoverTooltip } from '@/utils/equityCandleChart'

describe('EquityCandleTooltip', () => {
  it('优先展示持仓和本K线事件，不再渲染冗余OHLC和指标宫格', () => {
    const wrapper = mount(EquityCandleTooltip, {
      props: { tooltip: tooltip() },
    })

    expect(wrapper.find('.ecc-tooltip-ohlc').exists()).toBe(false)
    expect(wrapper.find('.ecc-tooltip-grid').exists()).toBe(false)
    expect(wrapper.text()).toContain('当前持仓')
    expect(wrapper.text()).toContain('BTC-USDT-SWAP')
    expect(wrapper.text()).toContain('本K线事件')
    expect(wrapper.text()).toContain('平多')
    expect(wrapper.text()).not.toContain('现金')
    expect(wrapper.text()).not.toContain('杠杆')
    expect(wrapper.text()).not.toContain('快照')
  })
})

function tooltip(): HoverTooltip {
  return {
    x: 12,
    y: 16,
    time: '04-24 16:00',
    open: '4,645.55',
    high: '4,672.03',
    low: '4,637.33',
    close: '4,667.23',
    change: '+0.47%',
    positive: true,
    equity: '4,667.23',
    cash: '4,639.49',
    notional: '3,508.22',
    unrealized: '+27.7395',
    unrealizedClass: 'positive',
    position: '多',
    positionDetail: '多 · 3,508.22',
    positionClass: 'positive',
    exposure: '75.2%',
    leverage: '3x',
    count: 16,
    positionTitle: '当前持仓',
    positionEmpty: '空仓',
    positionsTotal: 1,
    positionsMore: '',
    positions: [
      {
        key: 'btc',
        symbol: 'BTC-USDT-SWAP',
        side: '多单',
        sideClass: 'positive',
        quantity: '1',
        entryPrice: '100',
        markPrice: '105',
        notional: '105',
        pnl: '+5',
        pnlClass: 'positive',
        returnPct: '+5.00%',
        returnClass: 'positive',
      },
    ],
    eventTitle: '本K线事件',
    events: [
      {
        key: 'event',
        time: '04-24 16:00',
        symbol: 'BTC',
        label: '平多',
        sideClass: 'positive',
        pnl: '+5',
        pnlClass: 'positive',
      },
    ],
  }
}
