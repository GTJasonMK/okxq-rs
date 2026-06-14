import { describe, expect, it } from 'vitest'
import {
  aggregateCandles,
  aggregateSortedCandlesForRange,
  candleLimitForRange,
  candleRangeOptionsForTimeframe,
  chartRightPaddingForLatestAnchor,
  clampCandleRangeDaysForTimeframe,
  filterCandlesByRange,
  filterSortedCandlesByRange,
  latestAnchoredVisibleLogicalRange,
  mergeBookSide,
  mergeCandles,
  normalizeCandleRangeDays,
  marketSettingsPayload,
  normalizeMarketSettings,
  orderbookDisplaySide,
  sortedOrderbookSide,
} from '@/utils/marketView'
import type { Candle } from '@/types'

describe('行情页 K 线范围工具', () => {
  it('按周期和天数换算请求 K 线数量', () => {
    expect(candleLimitForRange('15m', 7)).toBe(672)
    expect(candleLimitForRange('1m', 30)).toBe(43_200)
    expect(candleLimitForRange('1m', 90)).toBe(100_000)
    expect(candleLimitForRange('1D', 30)).toBe(30)
  })

  it('只显示所选时间范围内的 K 线', () => {
    const candles = Array.from({ length: 120 }, (_, index) => candle(index))

    const visible = filterCandlesByRange(candles, '15m', 1)

    expect(visible).toHaveLength(96)
    expect(visible[0].timestamp).toBe(candle(24).timestamp)
    expect(visible.at(-1)?.timestamp).toBe(candle(119).timestamp)
  })

  it('范围筛选忽略非有限时间戳 K 线', () => {
    const candles = [
      ...Array.from({ length: 120 }, (_, index) => candle(index)),
      { ...candle(999), timestamp: Number.POSITIVE_INFINITY },
    ]

    const visible = filterCandlesByRange(candles, '15m', 1)

    expect(visible).toHaveLength(96)
    expect(visible[0].timestamp).toBe(candle(24).timestamp)
    expect(visible.at(-1)?.timestamp).toBe(candle(119).timestamp)
  })

  it('范围筛选忽略价格字段无效的 K 线', () => {
    const candles = [
      ...Array.from({ length: 120 }, (_, index) => candle(index)),
      { ...candle(120), open: Number.NaN },
    ]

    const visible = filterCandlesByRange(candles, '15m', 1)

    expect(visible).toHaveLength(96)
    expect(visible[0].timestamp).toBe(candle(24).timestamp)
    expect(visible.at(-1)?.timestamp).toBe(candle(119).timestamp)
    expect(visible.some(row => Number.isNaN(row.open))).toBe(false)
  })

  it('有序 K 线热路径与安全范围筛选保持一致', () => {
    const candles = Array.from({ length: 3 * 24 * 60 }, (_, index) => candle(index))

    expect(filterSortedCandlesByRange(candles, '1m', 1)).toEqual(
      filterCandlesByRange(candles, '1m', 1),
    )
    expect(
      filterSortedCandlesByRange(
        aggregateSortedCandlesForRange(candles, '1H', 1),
        '1H',
        1,
      ),
    ).toEqual(filterCandlesByRange(aggregateCandles(candles, '1H'), '1H', 1))
  })

  it('聚合 K 线按时间顺序计算开收盘价', () => {
    const base = candle(0).timestamp
    const candles: Candle[] = [
      { ...candle(0), timestamp: base + 10 * 60_000, open: 108, high: 112, low: 107, close: 110, volume: 3 },
      { ...candle(0), timestamp: base, open: 100, high: 103, low: 99, close: 102, volume: 2 },
      { ...candle(0), timestamp: base + 5 * 60_000, open: 102, high: 106, low: 101, close: 105, volume: 4 },
    ]

    expect(aggregateCandles(candles, '15m')).toEqual([{
      ...candles[1],
      timeframe: '15m',
      timestamp: base,
      open: 100,
      high: 112,
      low: 99,
      close: 110,
      volume: 9,
    }])
  })

  it('聚合 K 线忽略 OHLC 或成交量无效的输入行', () => {
    const base = candle(0).timestamp
    const candles: Candle[] = [
      { ...candle(0), timestamp: base, open: 100, high: 103, low: 99, close: 102, volume: 2 },
      { ...candle(0), timestamp: base + 5 * 60_000, open: 102, high: Number.NaN, low: 101, close: 105, volume: 4 },
      { ...candle(0), timestamp: base + 10 * 60_000, open: 108, high: 112, low: 107, close: 110, volume: 3 },
    ]

    expect(aggregateCandles(candles, '15m')).toEqual([{
      ...candles[0],
      timeframe: '15m',
      timestamp: base,
      open: 100,
      high: 112,
      low: 99,
      close: 110,
      volume: 5,
    }])
  })

  it('合并实时派生 K 线时无效行不会覆盖已有有效行', () => {
    const stored = [
      candle(0, { close: 100 }),
      candle(1, { close: 101 }),
    ]
    const realtimeDerived = [
      { ...candle(1, { close: 120 }), close: Number.NaN },
      candle(2, { close: 102 }),
    ]

    expect(mergeCandles(stored, realtimeDerived).map(row => [row.timestamp, row.close])).toEqual([
      [candle(0).timestamp, 100],
      [candle(1).timestamp, 101],
      [candle(2).timestamp, 102],
    ])
  })

  it('合并 K 线时实时同时间戳覆盖本地，重复时间戳取最后一条', () => {
    const stored = [
      candle(0, { close: 100 }),
      candle(1, { close: 101 }),
      candle(1, { close: 111 }),
      candle(3, { close: 103 }),
    ]
    const realtimeDerived = [
      candle(1, { close: 120 }),
      candle(1, { close: 121 }),
      candle(2, { close: 102 }),
    ]

    expect(mergeCandles(stored, realtimeDerived).map(row => [row.timestamp, row.close])).toEqual([
      [candle(0).timestamp, 100],
      [candle(1).timestamp, 121],
      [candle(2).timestamp, 102],
      [candle(3).timestamp, 103],
    ])
  })

  it('支持从路由和偏好中解析范围别名', () => {
    expect(normalizeCandleRangeDays('7d')).toBe(7)
    expect(normalizeCandleRangeDays('30天')).toBe(30)
    expect(normalizeCandleRangeDays('180天')).toBe(180)
    expect(normalizeCandleRangeDays('2y')).toBe(730)
    expect(normalizeCandleRangeDays('5年')).toBe(1825)
    expect(normalizeCandleRangeDays(365)).toBe(365)
    expect(normalizeCandleRangeDays('2d')).toBeUndefined()
  })

  it('按周期隐藏超过图表容量的范围，并向下夹紧', () => {
    expect(candleRangeOptionsForTimeframe('1m').map(item => item.value)).toEqual([1, 3, 7, 14, 30])
    expect(clampCandleRangeDaysForTimeframe(90, '1m')).toBe(30)
    expect(clampCandleRangeDaysForTimeframe(1825, '1H')).toBe(1825)
  })

  it('行情偏好只读取和写入当前字段', () => {
    expect(normalizeMarketSettings({
      activeSymbol: 'btc-usdt',
      marketInstType: 'SWAP',
      activeTimeframe: '15m',
      orderbookDepth: 250,
      candleRangeDays: 30,
      symbol: 'ETH-USDT',
      activeMarketType: 'SPOT',
      currentTimeframe: '1H',
      orderBookDepthLimit: 100,
      candleRange: 7,
    })).toEqual({
      activeSymbol: 'BTC-USDT',
      marketInstType: 'SWAP',
      activeTimeframe: '15m',
      orderbookDepth: 250,
      candleRangeDays: 30,
    })

    expect(normalizeMarketSettings({
      symbol: 'ETH-USDT',
      activeMarketType: 'SPOT',
      currentTimeframe: '1H',
      orderBookDepthLimit: 100,
      candleRange: 7,
    })).toEqual({
      activeSymbol: undefined,
      marketInstType: undefined,
      activeTimeframe: undefined,
      orderbookDepth: 400,
      candleRangeDays: undefined,
    })

    expect(marketSettingsPayload({
      activeSymbol: 'BTC-USDT',
      marketInstType: 'SWAP',
      activeTimeframe: '15m',
      orderbookDepth: 250,
      candleRangeDays: 30,
    })).toEqual({
      activeSymbol: 'BTC-USDT',
      marketInstType: 'SWAP',
      activeTimeframe: '15m',
      orderbookDepth: 250,
      candleRangeDays: 30,
    })
  })

  it('右侧留白让最新 K 线停在接近中部', () => {
    expect(chartRightPaddingForLatestAnchor(120)).toBe(120)

    const range = latestAnchoredVisibleLogicalRange(1000, 120)

    expect(range).toEqual({ from: 880, to: 1119, rightPadding: 120 })
    expect((999 - range!.from) / (range!.to - range!.from)).toBeCloseTo(0.5, 2)
  })

  it('盘口合并忽略非有限和非正数档位', () => {
    expect(mergeBookSide(
      [
        { price: Number.POSITIVE_INFINITY, size: 1, count: 1 },
        { price: 100, size: 2, count: 1 },
      ],
      [
        { price: 99, size: 3, count: 1 },
        { price: Number.NaN, size: 4, count: 1 },
        { price: 98, size: 0, count: 1 },
      ],
      'bid',
    )).toEqual([
      { price: 100, size: 2, count: 1 },
      { price: 99, size: 3, count: 1 },
    ])
  })

  it('盘口展示行保持全量排序后取可见档位的语义', () => {
    const rows = [
      { price: 100, size: 2, count: 1 },
      { price: Number.POSITIVE_INFINITY, size: 1, count: 1 },
      { price: 104, size: 5, count: 1 },
      { price: 101, size: 3, count: 1 },
      { price: 99, size: 4, count: 1 },
      { price: 102, size: 0, count: 1 },
      { price: 103, size: 1, count: 1 },
    ]
    const expectedBid = sortedOrderbookSide(rows, 'bid').slice(0, 3)
    const expectedAsk = sortedOrderbookSide(rows, 'ask').slice(0, 3).reverse()
    expect(sortedOrderbookSide(rows, 'bid').map(row => row.price)).toEqual([104, 103, 101, 100, 99])
    expect(sortedOrderbookSide(rows, 'ask').map(row => row.price)).toEqual([99, 100, 101, 103, 104])

    const bid = orderbookDisplaySide(rows, 'bid', 3)
    const ask = orderbookDisplaySide(rows, 'ask', 3)

    expect(bid.best?.price).toBe(expectedBid[0].price)
    expect(ask.best?.price).toBe(sortedOrderbookSide(rows, 'ask')[0].price)
    expect(bid.rows.map(row => row.price)).toEqual(expectedBid.map(row => row.price))
    expect(ask.rows.map(row => row.price)).toEqual(expectedAsk.map(row => row.price))
    expect(bid.rows.map(row => row.depthPct)).toEqual([100, 20, 60])
    expect(ask.rows.map(row => row.depthPct)).toEqual([75, 50, 100])
  })

})

function candle(index: number, overrides: Partial<Candle> = {}): Candle {
  const base = Date.UTC(2026, 0, 1, 0, 0, 0)
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    timestamp: base + index * 15 * 60_000,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: 1,
    ...overrides,
  }
}
