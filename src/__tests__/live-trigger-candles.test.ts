import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { defineComponent, h, nextTick, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import * as marketApi from '@/api/market'
import * as marketRealtimeApi from '@/api/marketRealtime'
import { useLiveTriggerCandles } from '@/composables/useLiveTriggerCandles'
import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'

vi.mock('@/api/market', () => ({
  fetchCandles: vi.fn(),
}))

vi.mock('@/api/marketRealtime', () => ({
  subscribeCandle: vi.fn(),
  unsubscribeCandle: vi.fn(),
}))

const fetchCandlesMock = vi.mocked(marketApi.fetchCandles)
const subscribeCandleMock = vi.mocked(marketRealtimeApi.subscribeCandle)
const unsubscribeCandleMock = vi.mocked(marketRealtimeApi.unsubscribeCandle)
const listenMock = vi.mocked(listen)

describe('useLiveTriggerCandles', () => {
  beforeEach(() => {
    fetchCandlesMock.mockResolvedValue([])
    subscribeCandleMock.mockResolvedValue(undefined)
    unsubscribeCandleMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('loads selected trigger candles with derived market params', async () => {
    const candles = [candle({ timestamp: 1_700_000_000_000 })]
    fetchCandlesMock.mockResolvedValueOnce(candles)
    const { view, wrapper } = mountLiveTriggerCandles()
    await flushPromises()

    await view.loadTriggerCandles()

    expect(fetchCandlesMock).toHaveBeenCalledWith('BTC-USDT-SWAP', {
      inst_type: 'SWAP',
      timeframe: '1m',
      limit: 1440,
    })
    expect(view.triggerCandles.value).toEqual(candles)

    wrapper.unmount()
    await flushPromises()
  })

  it('merges only current realtime candles and releases the subscription', async () => {
    const unlisten = vi.fn()
    const candleListener: {
      current?: (event: { payload: Record<string, unknown> }) => void
    } = {}
    listenMock.mockImplementation(async (_event, handler) => {
      candleListener.current = handler as typeof candleListener.current
      return unlisten
    })
    const { onRealtimeCandle, view, wrapper } = mountLiveTriggerCandles()
    await waitFor(() => !!candleListener.current)
    await waitFor(() => subscribeCandleMock.mock.calls.length > 0)

    candleListener.current?.({
      payload: realtimeCandle({ inst_id: 'ETH-USDT-SWAP' }),
    })
    expect(view.triggerCandles.value).toHaveLength(0)

    candleListener.current?.({
      payload: realtimeCandle({ confirm: undefined }),
    })

    expect(view.latestRealtimeTriggerCandle.value).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      confirm: '0',
    })
    expect(onRealtimeCandle).toHaveBeenCalledWith(false)

    candleListener.current?.({
      payload: realtimeCandle({ confirm: '1' }),
    })

    expect(view.triggerCandles.value).toMatchObject([
      { inst_id: 'BTC-USDT-SWAP', timeframe: '1m', timestamp: 1_700_000_000_000 },
    ])
    expect(view.latestRealtimeTriggerCandle.value).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      volume_ccy: 100,
      volume_quote: 100,
      confirm: '1',
    })
    expect(onRealtimeCandle).toHaveBeenLastCalledWith(true)

    wrapper.unmount()
    await flushPromises()

    expect(unlisten).toHaveBeenCalled()
    expect(unsubscribeCandleMock).toHaveBeenCalledWith('BTC-USDT-SWAP', '1m')
  })

  it('resubscribes when the selected symbol changes', async () => {
    const { selectedSymbol, wrapper } = mountLiveTriggerCandles()
    await waitFor(() => subscribeCandleMock.mock.calls.length >= 1)

    selectedSymbol.value = 'ETH-USDT-SWAP'
    await nextTick()
    await waitFor(() => subscribeCandleMock.mock.calls.length >= 2)

    expect(unsubscribeCandleMock).toHaveBeenCalledWith('BTC-USDT-SWAP', '1m')
    expect(subscribeCandleMock).toHaveBeenCalledWith('ETH-USDT-SWAP', '1m')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeCandleMock).toHaveBeenCalledWith('ETH-USDT-SWAP', '1m')
  })

  it('cleans up a realtime listener that resolves after unmount', async () => {
    const unlisten = vi.fn()
    let resolveListen!: (unlisten: () => void) => void
    listenMock.mockImplementation(() => new Promise((resolve) => {
      resolveListen = resolve
    }))
    const { wrapper } = mountLiveTriggerCandles()

    wrapper.unmount()
    resolveListen(unlisten)
    await flushPromises()

    expect(unlisten).toHaveBeenCalledTimes(1)
    expect(subscribeCandleMock).not.toHaveBeenCalled()
  })
})

function mountLiveTriggerCandles(options: {
  symbol?: string
  timeframe?: Timeframe
  rangeDays?: CandleRangeDays
} = {}) {
  const selectedSymbol = ref(options.symbol ?? 'BTC-USDT-SWAP')
  const timeframe = ref<Timeframe>(options.timeframe ?? '1m')
  const rangeDays = ref<CandleRangeDays>(options.rangeDays ?? 1)
  const onRealtimeCandle = vi.fn()
  const onRealtimeError = vi.fn()
  let view!: ReturnType<typeof useLiveTriggerCandles>
  const wrapper = mount(defineComponent({
    setup() {
      view = useLiveTriggerCandles({
        selectedSymbol,
        timeframe,
        rangeDays,
        onRealtimeCandle,
        onRealtimeError,
      })
      return () => h('div')
    },
  }))
  return {
    onRealtimeCandle,
    onRealtimeError,
    rangeDays,
    selectedSymbol,
    timeframe,
    view,
    wrapper,
  }
}

function candle(overrides: Partial<Candle> = {}): Candle {
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

function realtimeCandle(overrides: Record<string, unknown> = {}) {
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
    volume_ccy: 100,
    volume_quote: 100,
    confirm: '0',
    ...overrides,
  }
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 10; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
