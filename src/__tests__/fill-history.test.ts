import { nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FillHistory from '@/components/trading/FillHistory.vue'
import type { Fill } from '@/types'

const ROW_HEIGHT = 34

describe('FillHistory', () => {
  it('小成交列表保持完整渲染', () => {
    const fills = perfFills(30)
    const wrapper = mount(FillHistory, {
      props: { fills },
    })

    expect(wrapper.findAll('tbody tr')).toHaveLength(fills.length)
    expect(wrapper.find('tbody tr .symbol-cell').text()).toBe('BTC-USDT-SWAP')

    wrapper.unmount()
  })

  it('成交列表只挂载视窗附近行，避免私有成交事件后全量 DOM 重渲染', () => {
    const fills = perfFills(100)

    const wrapper = mount(FillHistory, {
      props: { fills },
    })

    expect(wrapper.findAll('tbody tr').length).toBeLessThanOrEqual(24)
    expect(wrapper.find('tbody tr .symbol-cell').text()).toBe('BTC-USDT-SWAP')

    wrapper.unmount()
  })

  it('成交列表滚动时切换可见窗口并保持挂载行数有界', async () => {
    const fills = perfFills(100)
    const wrapper = mount(FillHistory, {
      props: { fills },
    })
    const viewport = wrapper.find('.table-wrap')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = ROW_HEIGHT * 50
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('60050.00')
    expect(wrapper.findAll('.symbol-cell').length).toBeLessThanOrEqual(18)

    wrapper.unmount()
  })
})

function perfFills(count: number): Fill[] {
  const base = Date.parse('2026-06-06T00:00:00.000Z')
  return Array.from({ length: count }, (_, index) => ({
    fill_id: `fill-${String(index).padStart(3, '0')}`,
    inst_id: index % 2 === 0 ? 'BTC-USDT-SWAP' : 'ETH-USDT-SWAP',
    ord_id: `order-${index}`,
    side: index % 3 === 0 ? 'buy' : 'sell',
    fill_px: 60_000 + index,
    fill_sz: 0.01 + index / 10_000,
    fee: -0.02 - index / 1000,
    fee_ccy: 'USDT',
    fill_time: base - index * 1000,
  }))
}
