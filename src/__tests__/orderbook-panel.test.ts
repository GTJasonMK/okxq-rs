import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import OrderbookPanel from '@/components/market/OrderbookPanel.vue'
import type { Orderbook } from '@/types'

describe('OrderbookPanel', () => {
  it('忽略非有限盘口档位后计算价差', () => {
    const wrapper = mount(OrderbookPanel, {
      props: {
        orderbook: {
          inst_id: 'BTC-USDT-SWAP',
          bids: [
            { price: Number.POSITIVE_INFINITY, size: 1, count: 1 },
            { price: 100, size: 2, count: 1 },
          ],
          asks: [
            { price: 101, size: 3, count: 1 },
            { price: Number.NaN, size: 1, count: 1 },
          ],
          ts: 1_700_000_000_000,
        } satisfies Orderbook,
      },
    })

    expect(wrapper.find('.spread-text').text()).toBe('1.00 (0.9950%)')
  })
})
