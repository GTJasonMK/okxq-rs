import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useMarketStore } from '@/stores/marketStore'
import type { Candle } from '@/types'

describe('market store candle updates', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('maintains sorted upsert semantics while trimming to the newest window', () => {
    const store = useMarketStore()
    store.setCandles('BTC-USDT-SWAP', '1m', [candle(2), candle(0), candle(1)])

    expect(storedRows(store).map(row => row.timestamp)).toEqual([
      candle(0).timestamp,
      candle(1).timestamp,
      candle(2).timestamp,
    ])

    store.upsertCandle(candle(1, { close: 150 }), 3)
    expect(storedRows(store).map(row => row.close)).toEqual([100, 150, 100])

    store.upsertCandle(candle(3), 3)
    expect(storedRows(store).map(row => row.timestamp)).toEqual([
      candle(1).timestamp,
      candle(2).timestamp,
      candle(3).timestamp,
    ])

    store.upsertCandle(candle(0, { close: 80 }), 3)
    expect(storedRows(store).map(row => row.timestamp)).toEqual([
      candle(1).timestamp,
      candle(2).timestamp,
      candle(3).timestamp,
    ])

    store.upsertCandle(candleAt(candle(1).timestamp + 30_000), 3)
    expect(storedRows(store).map(row => row.timestamp)).toEqual([
      candle(1).timestamp + 30_000,
      candle(2).timestamp,
      candle(3).timestamp,
    ])
  })

  it('ignores invalid realtime candles without disturbing existing rows', () => {
    const store = useMarketStore()
    store.setCandles('BTC-USDT-SWAP', '1m', [candle(0), candle(1)])

    store.upsertCandle({ ...candle(2), open: Number.NaN }, 3)

    expect(storedRows(store).map(row => row.timestamp)).toEqual([
      candle(0).timestamp,
      candle(1).timestamp,
    ])
  })
})

function storedRows(store: ReturnType<typeof useMarketStore>) {
  return store.candles.get('BTC-USDT-SWAP:1m') ?? []
}

function candle(index: number, overrides: Partial<Candle> = {}): Candle {
  return candleAt(Date.UTC(2026, 0, 1) + index * 60_000, overrides)
}

function candleAt(timestamp: number, overrides: Partial<Candle> = {}): Candle {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1m',
    timestamp,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: 1,
    volume_ccy: 1,
    volume_quote: 100,
    ...overrides,
  }
}
