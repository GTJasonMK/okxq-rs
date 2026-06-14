import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { defineComponent, h, nextTick, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as marketRealtime from '@/api/marketRealtime'
import { useRealtimeOrderbook } from '@/composables/useRealtimeOrderbook'
import { useRealtimeTicker } from '@/composables/useRealtimeTicker'
import { useRealtimeTrades } from '@/composables/useRealtimeTrades'
import { useMarketStore } from '@/stores/marketStore'

vi.mock('@/api/marketRealtime', () => ({
  subscribeTicker: vi.fn(),
  unsubscribeTicker: vi.fn(),
  subscribeOrderbook: vi.fn(),
  unsubscribeOrderbook: vi.fn(),
  subscribeTrades: vi.fn(),
  unsubscribeTrades: vi.fn(),
}))

const subscribeTickerMock = vi.mocked(marketRealtime.subscribeTicker)
const unsubscribeTickerMock = vi.mocked(marketRealtime.unsubscribeTicker)
const subscribeOrderbookMock = vi.mocked(marketRealtime.subscribeOrderbook)
const unsubscribeOrderbookMock = vi.mocked(marketRealtime.unsubscribeOrderbook)
const subscribeTradesMock = vi.mocked(marketRealtime.subscribeTrades)
const unsubscribeTradesMock = vi.mocked(marketRealtime.unsubscribeTrades)
const listenMock = vi.mocked(listen)

type RealtimeListener = (event: { payload: Record<string, unknown> }) => void

describe('其他实时行情订阅', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    subscribeTickerMock.mockResolvedValue(undefined)
    unsubscribeTickerMock.mockResolvedValue(undefined)
    subscribeOrderbookMock.mockResolvedValue(undefined)
    unsubscribeOrderbookMock.mockResolvedValue(undefined)
    subscribeTradesMock.mockResolvedValue(undefined)
    unsubscribeTradesMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('orderbook 快速切换 instId 时释放迟到完成的旧订阅', async () => {
    const firstSubscribe = deferred<void>()
    subscribeOrderbookMock.mockImplementation((instId) => {
      if (instId === 'BTC-USDT-SWAP') return firstSubscribe.promise
      return Promise.resolve(undefined)
    })
    const activeInstId = ref('BTC-USDT-SWAP')
    const wrapper = mountOrderbook(activeInstId)

    await waitFor(() => subscribeOrderbookMock.mock.calls.length >= 1)
    activeInstId.value = 'ETH-USDT-SWAP'
    await nextTick()
    await waitFor(() => subscribeOrderbookMock.mock.calls.length >= 2)

    firstSubscribe.resolve()
    await flushPromises()

    expect(unsubscribeOrderbookMock).toHaveBeenCalledWith('BTC-USDT-SWAP')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeOrderbookMock).toHaveBeenCalledWith('ETH-USDT-SWAP')
  })

  it('trades 快速切换 instId 时释放迟到完成的旧订阅', async () => {
    const firstSubscribe = deferred<void>()
    subscribeTradesMock.mockImplementation((instId) => {
      if (instId === 'BTC-USDT-SWAP') return firstSubscribe.promise
      return Promise.resolve(undefined)
    })
    const activeInstId = ref('BTC-USDT-SWAP')
    const wrapper = mountTrades(activeInstId)

    await waitFor(() => subscribeTradesMock.mock.calls.length >= 1)
    activeInstId.value = 'ETH-USDT-SWAP'
    await nextTick()
    await waitFor(() => subscribeTradesMock.mock.calls.length >= 2)

    firstSubscribe.resolve()
    await flushPromises()

    expect(unsubscribeTradesMock).toHaveBeenCalledWith('BTC-USDT-SWAP')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeTradesMock).toHaveBeenCalledWith('ETH-USDT-SWAP')
  })

  it('ticker 快速替换 symbol 列表时释放迟到完成的旧订阅', async () => {
    const firstSubscribe = deferred<void>()
    subscribeTickerMock.mockImplementation((instId) => {
      if (instId === 'BTC-USDT-SWAP') return firstSubscribe.promise
      return Promise.resolve(undefined)
    })
    const symbols = ref(['BTC-USDT-SWAP'])
    const wrapper = mountTicker(symbols)

    await waitFor(() => subscribeTickerMock.mock.calls.length >= 1)
    symbols.value = ['ETH-USDT-SWAP']
    await nextTick()
    await waitFor(() => subscribeTickerMock.mock.calls.length >= 2)

    firstSubscribe.resolve()
    await flushPromises()

    expect(unsubscribeTickerMock).toHaveBeenCalledWith('BTC-USDT-SWAP')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeTickerMock).toHaveBeenCalledWith('ETH-USDT-SWAP')
  })

  it('ticker 多 symbol 订阅只 acquire 唯一有效 instId', async () => {
    const symbols = ref(['BTC-USDT-SWAP', 'BTC-USDT-SWAP', '', 'ETH-USDT-SWAP'])
    const wrapper = mountTicker(symbols)

    await waitFor(() => subscribeTickerMock.mock.calls.length >= 2)

    expect(subscribeTickerMock.mock.calls.map(([instId]) => instId)).toEqual([
      'BTC-USDT-SWAP',
      'ETH-USDT-SWAP',
    ])

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeTickerMock.mock.calls.map(([instId]) => instId)).toEqual([
      'BTC-USDT-SWAP',
      'ETH-USDT-SWAP',
    ])
  })

  it('ticker 实时事件归一化 OKX payload 后写入 store', async () => {
    const listeners = captureRealtimeListeners()
    const symbols = ref(['BTC-USDT-SWAP'])
    const wrapper = mountTicker(symbols)
    await waitFor(() => !!listeners['okxq-market-ticker'])

    listeners['okxq-market-ticker']?.({
      payload: {
        instId: 'btc-usdt-swap',
        instType: 'swap',
        last: '110',
        askPx: '111',
        bidPx: '109',
        open24h: '100',
      },
    })

    expect(useMarketStore().tickers.get('BTC-USDT-SWAP')).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      ask: 111,
      bid: 109,
      change24h: 10,
    })

    wrapper.unmount()
  })

  it('ticker 实时事件忽略非法最新价', async () => {
    const listeners = captureRealtimeListeners()
    const symbols = ref(['BTC-USDT-SWAP'])
    const wrapper = mountTicker(symbols)
    await waitFor(() => !!listeners['okxq-market-ticker'])

    listeners['okxq-market-ticker']?.({
      payload: {
        instId: 'btc-usdt-swap',
        last: 'bad-last',
        askPx: '111',
        bidPx: '109',
        open24h: '100',
      },
    })

    expect(useMarketStore().tickers.get('BTC-USDT-SWAP')).toBeUndefined()

    wrapper.unmount()
  })

  it('orderbook 实时事件归一化 OKX 数组档位', async () => {
    const listeners = captureRealtimeListeners()
    const activeInstId = ref('BTC-USDT-SWAP')
    const { wrapper, result } = mountOrderbookWithState(activeInstId)
    await waitFor(() => !!listeners['okxq-market-orderbook'])

    listeners['okxq-market-orderbook']?.({
      payload: {
        instId: 'btc-usdt-swap',
        bids: [['100', '2', '0', '4']],
        asks: [['101', '3', '5']],
        ts: 1_700_000_000_000,
      },
    })

    expect(result.orderbook.value).toEqual({
      inst_id: 'BTC-USDT-SWAP',
      bids: [{ price: 100, size: 2, count: 4 }],
      asks: [{ price: 101, size: 3, count: 5 }],
      ts: 1_700_000_000_000,
    })

    wrapper.unmount()
  })

  it('orderbook 实时事件忽略非法或零数量档位', async () => {
    const listeners = captureRealtimeListeners()
    const activeInstId = ref('BTC-USDT-SWAP')
    const { wrapper, result } = mountOrderbookWithState(activeInstId)
    await waitFor(() => !!listeners['okxq-market-orderbook'])

    listeners['okxq-market-orderbook']?.({
      payload: {
        instId: 'btc-usdt-swap',
        bids: [
          ['100', 'bad-size', '0', '4'],
          ['99', '0', '0', '1'],
          ['98', '2', '0', '2'],
        ],
        asks: [['101', '3', '0', '5']],
        ts: 1_700_000_000_000,
      },
    })

    expect(result.orderbook.value).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      bids: [{ price: 98, size: 2, count: 2 }],
      asks: [{ price: 101, size: 3, count: 5 }],
      ts: 1_700_000_000_000,
    })

    wrapper.unmount()
  })

  it('trades 实时事件归一化 OKX 逐笔字段', async () => {
    const listeners = captureRealtimeListeners()
    const activeInstId = ref('SOL-USDT-SWAP')
    const { wrapper, result } = mountTradesWithState(activeInstId)
    await waitFor(() => !!listeners['okxq-market-trade'])

    listeners['okxq-market-trade']?.({
      payload: {
        instId: 'sol-usdt-swap',
        tradeId: 12345,
        px: '155.5',
        sz: '8',
        side: 'sell',
        ts: 1_700_000_000_000,
      },
    })

    expect(result.trades.value).toEqual([{
      inst_id: 'SOL-USDT-SWAP',
      trade_id: '12345',
      price: 155.5,
      size: 8,
      side: 'sell',
      ts: 1_700_000_000_000,
    }])

    wrapper.unmount()
  })

  it('trades 实时事件忽略非法价格或数量', async () => {
    const listeners = captureRealtimeListeners()
    const activeInstId = ref('SOL-USDT-SWAP')
    const { wrapper, result } = mountTradesWithState(activeInstId)
    await waitFor(() => !!listeners['okxq-market-trade'])

    listeners['okxq-market-trade']?.({
      payload: {
        instId: 'sol-usdt-swap',
        tradeId: 'bad-price',
        px: 'bad-price',
        sz: '8',
        side: 'sell',
        ts: 1_700_000_000_000,
      },
    })
    listeners['okxq-market-trade']?.({
      payload: {
        instId: 'sol-usdt-swap',
        tradeId: 'bad-size',
        px: '155.5',
        sz: 'bad-size',
        side: 'buy',
        ts: 1_700_000_000_001,
      },
    })

    expect(result.trades.value).toEqual([])

    wrapper.unmount()
  })
})

function mountOrderbook(activeInstId: { value: string }) {
  return mount(defineComponent({
    setup() {
      useRealtimeOrderbook(() => activeInstId.value)
      return () => h('div')
    },
  }))
}

function mountTrades(activeInstId: { value: string }) {
  return mount(defineComponent({
    setup() {
      useRealtimeTrades(() => activeInstId.value)
      return () => h('div')
    },
  }))
}

function mountTicker(symbols: { value: string[] }) {
  return mount(defineComponent({
    setup() {
      useRealtimeTicker(() => symbols.value)
      return () => h('div')
    },
  }))
}

function mountOrderbookWithState(activeInstId: { value: string }) {
  let result!: ReturnType<typeof useRealtimeOrderbook>
  const wrapper = mount(defineComponent({
    setup() {
      result = useRealtimeOrderbook(() => activeInstId.value)
      return () => h('div')
    },
  }))
  return { wrapper, result }
}

function mountTradesWithState(activeInstId: { value: string }) {
  let result!: ReturnType<typeof useRealtimeTrades>
  const wrapper = mount(defineComponent({
    setup() {
      result = useRealtimeTrades(() => activeInstId.value)
      return () => h('div')
    },
  }))
  return { wrapper, result }
}

function captureRealtimeListeners() {
  const listeners: Record<string, RealtimeListener> = {}
  listenMock.mockImplementation(async (event, handler) => {
    listeners[String(event)] = handler as RealtimeListener
    return () => {}
  })
  return listeners
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
  for (let index = 0; index < 10; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
