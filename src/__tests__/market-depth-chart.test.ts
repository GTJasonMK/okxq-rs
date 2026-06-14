import { describe, expect, it } from 'vitest'
import { effectScope } from 'vue'
import { useMarketDepthChart } from '@/composables/useMarketDepthChart'
import type { Orderbook } from '@/types'

describe('useMarketDepthChart', () => {
  it('价差使用有效买卖一档而不是原始数组首行', () => {
    const orderbook: Orderbook = {
      inst_id: 'BTC-USDT-SWAP',
      bids: [
        { price: 0, size: 0, count: 1 },
        { price: 100, size: 2, count: 1 },
      ],
      asks: [
        { price: 101, size: 3, count: 1 },
      ],
      ts: 1_700_000_000_000,
    }

    const scope = effectScope()
    const chart = scope.run(() => useMarketDepthChart({
      orderbook: () => orderbook,
      bidDepth: () => 40,
      askDepth: () => 40,
    }))

    expect(chart?.depthChart.value.hasData).toBe(true)
    expect(chart?.depthChart.value.bestBid).toBe(100)
    expect(chart?.depthChart.value.bestAsk).toBe(101)
    expect(chart?.spread.value).toBe(1)

    scope.stop()
  })

  it('深度图用完整深度计算统计但限制 SVG 绘图点数量', () => {
    const orderbook: Orderbook = {
      inst_id: 'BTC-USDT-SWAP',
      bids: Array.from({ length: 5000 }, (_, index) => ({
        price: 100_000 - index,
        size: 1,
        count: 1,
      })),
      asks: Array.from({ length: 5000 }, (_, index) => ({
        price: 100_001 + index,
        size: 1,
        count: 1,
      })),
      ts: 1_700_000_000_000,
    }

    const scope = effectScope()
    const chart = scope.run(() => useMarketDepthChart({
      orderbook: () => orderbook,
      bidDepth: () => 5000,
      askDepth: () => 5000,
    }))

    expect(chart?.depthChart.value.hasData).toBe(true)
    expect(chart?.depthChart.value.bidTotal).toBe(5000)
    expect(chart?.depthChart.value.askTotal).toBe(5000)
    expect(pointCount(chart?.depthChart.value.bidLinePoints ?? '')).toBeLessThan(650)
    expect(pointCount(chart?.depthChart.value.askLinePoints ?? '')).toBeLessThan(650)

    scope.stop()
  })
})

function pointCount(points: string) {
  return points.split(' ').filter(Boolean).length
}
