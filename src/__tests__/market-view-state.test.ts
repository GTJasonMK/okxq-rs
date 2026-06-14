import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as marketApi from '@/api/market'
import { useMarketViewState } from '@/composables/useMarketViewState'
import { useMarketStore } from '@/stores/marketStore'
import type { Ticker, WatchedSymbol } from '@/types'

const routeState = vi.hoisted(() => ({
  query: {} as Record<string, unknown>,
}))
const routerPushMock = vi.hoisted(() => vi.fn())

vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({ push: routerPushMock }),
}))

vi.mock('@/composables/useMarketPreferences', () => ({
  useMarketPreferences: () => ({
    loadMarketPreferences: vi.fn().mockResolvedValue(undefined),
    scheduleSaveMarketPreferences: vi.fn(),
    flushMarketPreferences: vi.fn(),
  }),
}))

vi.mock('@/composables/useRealtimeTicker', async () => {
  const { ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    useRealtimeTicker: vi.fn(() => ({ connected: ref(false), error: ref(null) })),
  }
})

vi.mock('@/composables/useRealtimeCandle', async () => {
  const { ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    useRealtimeCandle: vi.fn(() => ({ connected: ref(false), error: ref(null) })),
  }
})

vi.mock('@/composables/useRealtimeOrderbook', async () => {
  const { ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    useRealtimeOrderbook: vi.fn(() => ({
      orderbook: ref(null),
      connected: ref(false),
      error: ref(null),
    })),
  }
})

vi.mock('@/composables/useRealtimeTrades', async () => {
  const { ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    useRealtimeTrades: vi.fn(() => ({
      trades: ref([]),
      connected: ref(false),
      error: ref(null),
    })),
  }
})

vi.mock('@/api/market', async () => {
  const actual = await vi.importActual<typeof import('@/api/market')>('@/api/market')
  return {
    ...actual,
    fetchWatchedSymbols: vi.fn(),
    fetchCandles: vi.fn(),
    fetchTicker: vi.fn(),
    fetchOrderbook: vi.fn(),
    fetchRecentTrades: vi.fn(),
  }
})

const fetchWatchedSymbolsMock = vi.mocked(marketApi.fetchWatchedSymbols)
const fetchCandlesMock = vi.mocked(marketApi.fetchCandles)
const fetchTickerMock = vi.mocked(marketApi.fetchTicker)
const fetchOrderbookMock = vi.mocked(marketApi.fetchOrderbook)
const fetchRecentTradesMock = vi.mocked(marketApi.fetchRecentTrades)

describe('useMarketViewState', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    routeState.query = {}
    routerPushMock.mockClear()
    fetchWatchedSymbolsMock.mockResolvedValue([
      watchedSymbol({ symbol: 'BTC-USDT', base_ccy: 'BTC' }),
      watchedSymbol({ symbol: 'ETH-USDT', base_ccy: 'ETH' }),
    ])
    fetchCandlesMock.mockResolvedValue([])
    fetchOrderbookMock.mockResolvedValue({
      inst_id: 'BTC-USDT-SWAP',
      bids: [],
      asks: [],
      ts: 0,
    })
    fetchRecentTradesMock.mockResolvedValue([])
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('A→B→A 快速切换时忽略旧 A 快照响应', async () => {
    const oldBtcTickers: Array<ReturnType<typeof deferred<Ticker>>> = []
    const latestBtcTicker = deferred<Ticker>()
    let useLatestBtcTicker = false

    fetchTickerMock.mockImplementation((instId) => {
      if (instId === 'BTC-USDT-SWAP') {
        if (useLatestBtcTicker) return latestBtcTicker.promise
        const pending = deferred<Ticker>()
        oldBtcTickers.push(pending)
        return pending.promise
      }
      return Promise.resolve(ticker({ inst_id: 'ETH-USDT-SWAP', last: 300 }))
    })

    const { view, wrapper } = mountMarketViewState()
    await waitFor(() => oldBtcTickers.length > 0)

    view.handleSymbolUpdate('ETH-USDT')
    await nextTick()
    await waitFor(() => fetchTickerMock.mock.calls.some(([instId]) => instId === 'ETH-USDT-SWAP'))

    useLatestBtcTicker = true
    view.handleSymbolUpdate('BTC-USDT')
    await nextTick()
    await waitFor(() => fetchTickerMock.mock.calls.filter(([instId]) => instId === 'BTC-USDT-SWAP').length > oldBtcTickers.length)

    latestBtcTicker.resolve(ticker({ inst_id: 'BTC-USDT-SWAP', last: 200 }))
    await flushPromises()

    expect(useMarketStore().tickers.get('BTC-USDT-SWAP')?.last).toBe(200)

    for (const pending of oldBtcTickers) {
      pending.resolve(ticker({ inst_id: 'BTC-USDT-SWAP', last: 100 }))
    }
    await flushPromises()

    expect(useMarketStore().tickers.get('BTC-USDT-SWAP')?.last).toBe(200)

    wrapper.unmount()
  })

  it('按需请求更深盘口并复用已有足够深度的快照', async () => {
    const { view, wrapper } = mountMarketViewState()
    await waitFor(() => fetchOrderbookMock.mock.calls.length > 0)

    fetchOrderbookMock.mockClear()
    await view.handleDepthRequest(800)
    await flushPromises()

    expect(fetchOrderbookMock).toHaveBeenCalledWith('BTC-USDT-SWAP', 800, 'SWAP')

    fetchOrderbookMock.mockClear()
    await view.handleDepthRequest(200)
    await flushPromises()

    expect(fetchOrderbookMock).not.toHaveBeenCalled()

    wrapper.unmount()
  })
})

function mountMarketViewState() {
  let view!: ReturnType<typeof useMarketViewState>
  const wrapper = mount(defineComponent({
    setup() {
      view = useMarketViewState()
      return () => h('div')
    },
  }), {
    global: {
      plugins: [createPinia()],
    },
  })
  return { view, wrapper }
}

function watchedSymbol(overrides: Partial<WatchedSymbol> = {}): WatchedSymbol {
  const baseCcy = overrides.base_ccy ?? 'BTC'
  const symbol = overrides.symbol ?? `${baseCcy}-USDT`
  return {
    symbol,
    base_ccy: baseCcy,
    spot_inst_id: symbol,
    swap_inst_id: `${symbol}-SWAP`,
    sync_spot: false,
    sync_swap: true,
    ...overrides,
  }
}

function ticker(overrides: Partial<Ticker> = {}): Ticker {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    last: 100,
    ask: 100,
    bid: 100,
    open24h: 100,
    high24h: 100,
    low24h: 100,
    vol24h: 0,
    change24h: 0,
    ts: 1_700_000_000_000,
    ...overrides,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 20; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
