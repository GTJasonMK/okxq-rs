import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import TradingView from '@/views/TradingView.vue'

vi.mock('@/composables/useTradingView', async () => {
  const { reactive, ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    useTradingView: () => ({
      store: reactive({
        account: {
          details: [
            { ccy: 'USDT', total: 100, available: 100, frozen: 0 },
            { ccy: 'BTC', total: 0.1, available: 0.1, frozen: 0 },
          ],
        },
        positions: [{ inst_id: 'BTC-USDT-SWAP', pos_side: 'short', pos: -1 }],
        orders: [
          { ord_id: 'o1', inst_id: 'BTC-USDT-SWAP' },
          { ord_id: 'o2', inst_id: 'ETH-USDT-SWAP' },
        ],
        fills: [{ fill_id: 'f1', inst_id: 'BTC-USDT-SWAP' }],
      }),
      systemStore: { tradingModeLabel: '模拟盘' },
      loading: ref(false),
      error: ref(''),
      message: ref(''),
      privateRealtimeConnected: ref(true),
      privateRealtimeError: ref(''),
      privateRealtimeMode: ref('simulated'),
      closingPositionKeys: ref(new Set<string>()),
      viewMode: ref('simulated'),
      viewModeLabel: ref('模拟盘'),
      viewModeLocked: ref(false),
      refreshAll: vi.fn(),
      handleOrderSubmitted: vi.fn(),
      handleOrderCancelled: vi.fn(),
      handleClosePosition: vi.fn(),
    }),
  }
})

describe('TradingView 布局', () => {
  it('右侧资产、持仓和成交使用 tab 切换，默认显示持仓', async () => {
    const wrapper = mount(TradingView, {
      global: {
        stubs: {
          AccountSummary: { template: '<section data-testid="summary" />' },
          OrderForm: { template: '<section data-testid="order-form" />' },
          PendingOrders: { template: '<section data-testid="pending-orders" />' },
          PositionTable: { template: '<section data-testid="positions-panel">持仓面板</section>' },
          AssetHoldingsTable: { template: '<section data-testid="assets-panel">资产面板</section>' },
          FillHistory: { template: '<section data-testid="fills-panel">成交面板</section>' },
        },
      },
    })

    const actionTabs = wrapper.findAll('.vt-action-tab')
    expect(actionTabs.map(tab => tab.text())).toEqual(['下单', '挂单2'])
    expect(actionTabs[0].classes()).toContain('active')
    expect(actionTabs[1].classes()).not.toContain('active')

    await actionTabs[1].trigger('click')
    await nextTick()

    expect(actionTabs[0].classes()).not.toContain('active')
    expect(actionTabs[1].classes()).toContain('active')

    const tabs = wrapper.findAll('.vt-info-tab')
    expect(tabs.map(tab => tab.text())).toEqual(['持仓1', '资产2', '成交1'])
    expect(wrapper.find('[data-testid="positions-panel"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="assets-panel"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="fills-panel"]').exists()).toBe(false)

    await tabs[1].trigger('click')
    await nextTick()

    expect(wrapper.find('[data-testid="positions-panel"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="assets-panel"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="fills-panel"]').exists()).toBe(false)

    await tabs[2].trigger('click')
    await nextTick()

    expect(wrapper.find('[data-testid="positions-panel"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="assets-panel"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="fills-panel"]').exists()).toBe(true)

    wrapper.unmount()
  })
})
