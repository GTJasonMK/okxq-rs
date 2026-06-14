import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { defineComponent, h, nextTick, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as marketRealtime from '@/api/marketRealtime'
import { useRealtimeCandle } from '@/composables/useRealtimeCandle'
import { useMarketStore } from '@/stores/marketStore'

vi.mock('@/api/marketRealtime', () => ({
  subscribeCandle: vi.fn(),
  unsubscribeCandle: vi.fn(),
}))

const subscribeCandleMock = vi.mocked(marketRealtime.subscribeCandle)
const unsubscribeCandleMock = vi.mocked(marketRealtime.unsubscribeCandle)
const listenMock = vi.mocked(listen)

describe('useRealtimeCandle', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    subscribeCandleMock.mockResolvedValue(undefined)
    unsubscribeCandleMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('快速切换 instId 时释放迟到完成的旧订阅', async () => {
    const firstSubscribe = deferred<void>()
    subscribeCandleMock.mockImplementation((instId) => {
      if (instId === 'BTC-USDT-SWAP') return firstSubscribe.promise
      return Promise.resolve(undefined)
    })
    const activeInstId = ref('BTC-USDT-SWAP')
    const wrapper = mountRealtimeCandle(activeInstId)

    await waitFor(() => subscribeCandleMock.mock.calls.length >= 1)
    expect(subscribeCandleMock).toHaveBeenCalledWith('BTC-USDT-SWAP', '1m')

    activeInstId.value = 'ETH-USDT-SWAP'
    await nextTick()
    await waitFor(() => subscribeCandleMock.mock.calls.length >= 2)
    expect(subscribeCandleMock).toHaveBeenCalledWith('ETH-USDT-SWAP', '1m')

    firstSubscribe.resolve()
    await flushPromises()

    expect(unsubscribeCandleMock).toHaveBeenCalledWith('BTC-USDT-SWAP', '1m')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeCandleMock).toHaveBeenCalledWith('ETH-USDT-SWAP', '1m')
  })

  it('只写入当前 instId 的 1m 实时 K 线', async () => {
    const candleListener: {
      current?: (event: { payload: Record<string, unknown> }) => void
    } = {}
    listenMock.mockImplementation(async (_event, handler) => {
      candleListener.current = handler as typeof candleListener.current
      return () => {}
    })
    const activeInstId = ref('BTC-USDT-SWAP')
    mountRealtimeCandle(activeInstId)
    await waitFor(() => !!candleListener.current)

    candleListener.current?.({ payload: candlePayload({ inst_id: 'ETH-USDT-SWAP', timestamp: 1_700_000_000_000 }) })
    candleListener.current?.({ payload: candlePayload({ inst_id: 'BTC-USDT-SWAP', timestamp: 1_700_000_060_000 }) })
    candleListener.current?.({ payload: candlePayload({ inst_id: 'BTC-USDT-SWAP', timeframe: '5m', timestamp: 1_700_000_120_000 }) })

    const store = useMarketStore()
    expect(store.candles.get('ETH-USDT-SWAP:1m')).toBeUndefined()
    expect(store.candles.get('BTC-USDT-SWAP:5m')).toBeUndefined()
    expect(store.candles.get('BTC-USDT-SWAP:1m')).toHaveLength(1)
    expect(store.candles.get('BTC-USDT-SWAP:1m')?.[0]?.timestamp).toBe(1_700_000_060_000)
  })

  it('忽略非正时间戳实时 K 线', async () => {
    const candleListener: {
      current?: (event: { payload: Record<string, unknown> }) => void
    } = {}
    listenMock.mockImplementation(async (_event, handler) => {
      candleListener.current = handler as typeof candleListener.current
      return () => {}
    })
    const activeInstId = ref('BTC-USDT-SWAP')
    mountRealtimeCandle(activeInstId)
    await waitFor(() => !!candleListener.current)

    candleListener.current?.({ payload: candlePayload({ timestamp: 0 }) })

    expect(useMarketStore().candles.get('BTC-USDT-SWAP:1m')).toBeUndefined()
  })
})

function mountRealtimeCandle(activeInstId: { value: string }) {
  const TestComponent = defineComponent({
    setup() {
      useRealtimeCandle(() => activeInstId.value)
      return () => h('div')
    },
  })
  return mount(TestComponent)
}

function candlePayload(overrides: Record<string, unknown> = {}) {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1m',
    timestamp: 1_700_000_000_000,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: 1,
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
  for (let index = 0; index < 10; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
