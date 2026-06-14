import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, type PropType } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as marketApi from '@/api/market'
import MarketView from '@/views/MarketView.vue'
import { candleLimitForRange } from '@/utils/marketView'
import type { Candle, Orderbook, Ticker, WatchedSymbol } from '@/types'

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

describe('MarketView 页面交互', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    routeState.query = {}
    routerPushMock.mockClear()
    fetchWatchedSymbolsMock.mockResolvedValue([
      watchedSymbol({ symbol: 'BTC-USDT', base_ccy: 'BTC' }),
      watchedSymbol({ symbol: 'ETH-USDT', base_ccy: 'ETH' }),
    ])
    fetchCandlesMock.mockResolvedValue([
      candle({ inst_id: 'BTC-USDT-SWAP' }),
    ])
    fetchTickerMock.mockResolvedValue(ticker())
    fetchOrderbookMock.mockResolvedValue(orderbook())
    fetchRecentTradesMock.mockResolvedValue([])
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('关注币种下拉和 K 线范围下拉驱动当前图表请求', async () => {
    const wrapper = mountMarketView()
    await waitFor(() => fetchCandlesMock.mock.calls.length > 0)

    expect(wrapper.find('.market-selector').exists()).toBe(true)
    expect(wrapper.find('[data-test="kline-chart"]').attributes('data-range-days')).toBe('7')

    const rangeSelect = wrapper.find('select[data-role="range"]')
    expect(rangeSelect.findAll('option').map(option => option.attributes('value'))).toContain('180')

    await rangeSelect.setValue('180')
    await waitFor(() =>
      fetchCandlesMock.mock.calls.some(([instId, params]) =>
        instId === 'BTC-USDT-SWAP' && Number(params?.limit) === candleLimitForRange('1H', 180)
      )
    )

    expect(wrapper.find('[data-test="kline-chart"]').attributes('data-range-days')).toBe('180')

    await wrapper.find('select[data-role="watch-symbol"]').setValue('ETH-USDT')
    await waitFor(() =>
      fetchCandlesMock.mock.calls.some(([instId]) => instId === 'ETH-USDT-SWAP')
    )

    expect(fetchTickerMock).toHaveBeenCalledWith('ETH-USDT-SWAP', 'SWAP')

    wrapper.unmount()
  })
})

function mountMarketView() {
  return mount(MarketView, {
    global: {
      plugins: [createPinia()],
      stubs: {
        ThemeSelect: ThemeSelectStub,
        KlineChart: defineComponent({
          name: 'KlineChart',
          props: {
            candles: { type: Array, default: () => [] },
            timeframe: { type: String, required: true },
            rangeDays: { type: Number, required: true },
          },
          setup(props) {
            return () => h('div', {
              'data-test': 'kline-chart',
              'data-timeframe': props.timeframe,
              'data-range-days': String(props.rangeDays),
            })
          },
        }),
        MarketControls: stubComponent('MarketControls'),
        MarketSyncProgress: stubComponent('MarketSyncProgress'),
        TickerBar: stubComponent('TickerBar'),
        OrderbookPanel: stubComponent('OrderbookPanel'),
        RecentTrades: stubComponent('RecentTrades'),
      },
    },
  })
}

const ThemeSelectStub = defineComponent({
  name: 'ThemeSelect',
  props: {
    modelValue: {
      type: String,
      default: '',
    },
    options: {
      type: Array as PropType<Array<{ value: string; label: string }>>,
      required: true,
    },
    placeholder: {
      type: String,
      default: '',
    },
    size: {
      type: String,
      default: 'md',
    },
  },
  emits: ['update:modelValue'],
  setup(props, { emit }) {
    const role = props.placeholder === '选择关注币种' ? 'watch-symbol' : 'range'
    return () => h('select', {
      'data-role': role,
      value: props.modelValue,
      onChange: (event: Event) => {
        emit('update:modelValue', (event.target as HTMLSelectElement).value)
      },
    }, props.options.map(option => h('option', { value: option.value }, option.label)))
  },
})

function stubComponent(name: string) {
  return defineComponent({
    name,
    setup(_, { slots }) {
      return () => h('div', { class: `stub-${name}` }, slots.default?.())
    },
  })
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

function candle(overrides: Partial<Candle> = {}): Candle {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    timestamp: 1_780_000_000_000,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: 1,
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
    ts: 1_780_000_000_000,
    ...overrides,
  }
}

function orderbook(overrides: Partial<Orderbook> = {}): Orderbook {
  return {
    inst_id: 'BTC-USDT-SWAP',
    bids: [],
    asks: [],
    ts: 1_780_000_000_000,
    ...overrides,
  }
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 20; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
