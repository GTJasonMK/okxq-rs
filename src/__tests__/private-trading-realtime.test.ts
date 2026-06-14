import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { defineComponent, h, nextTick, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import * as marketRealtime from '@/api/marketRealtime'
import { usePrivateTradingRealtime } from '@/composables/usePrivateTradingRealtime'
import { useTradingStore } from '@/stores/tradingStore'

vi.mock('@/api/marketRealtime', () => ({
  subscribeAccount: vi.fn(),
  unsubscribeAccount: vi.fn(),
  subscribeOrders: vi.fn(),
  unsubscribeOrders: vi.fn(),
  subscribeAlgoOrders: vi.fn(),
  unsubscribeAlgoOrders: vi.fn(),
  subscribeFills: vi.fn(),
  unsubscribeFills: vi.fn(),
  subscribePositions: vi.fn(),
  unsubscribePositions: vi.fn(),
}))

const listenMock = vi.mocked(listen)
const subscribeAccountMock = vi.mocked(marketRealtime.subscribeAccount)
const unsubscribeAccountMock = vi.mocked(marketRealtime.unsubscribeAccount)
const subscribeOrdersMock = vi.mocked(marketRealtime.subscribeOrders)
const unsubscribeOrdersMock = vi.mocked(marketRealtime.unsubscribeOrders)
const subscribeAlgoOrdersMock = vi.mocked(marketRealtime.subscribeAlgoOrders)
const unsubscribeAlgoOrdersMock = vi.mocked(marketRealtime.unsubscribeAlgoOrders)
const subscribeFillsMock = vi.mocked(marketRealtime.subscribeFills)
const unsubscribeFillsMock = vi.mocked(marketRealtime.unsubscribeFills)
const subscribePositionsMock = vi.mocked(marketRealtime.subscribePositions)
const unsubscribePositionsMock = vi.mocked(marketRealtime.unsubscribePositions)

type RealtimeListener = (event: { payload: Record<string, unknown> }) => void

describe('私有交易实时持仓', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    subscribeAccountMock.mockResolvedValue(undefined)
    unsubscribeAccountMock.mockResolvedValue(undefined)
    subscribeOrdersMock.mockResolvedValue(undefined)
    unsubscribeOrdersMock.mockResolvedValue(undefined)
    subscribeAlgoOrdersMock.mockResolvedValue(undefined)
    unsubscribeAlgoOrdersMock.mockResolvedValue(undefined)
    subscribeFillsMock.mockResolvedValue(undefined)
    unsubscribeFillsMock.mockResolvedValue(undefined)
    subscribePositionsMock.mockResolvedValue(undefined)
    unsubscribePositionsMock.mockResolvedValue(undefined)
    listenMock.mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('订阅 positions 并用私有持仓事件刷新标记价和未实现盈亏', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => subscribePositionsMock.mock.calls.length === 1)
    expect(subscribePositionsMock).toHaveBeenCalledWith('simulated')
    expect(listeners['okxq-private-position']).toBeTruthy()

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'simulated',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'short',
        pos: -2,
        avg_px: 70000,
        mark_px: 71000.25,
        upl: -2000.5,
        upl_ratio: -0.0285,
        lever: 5,
        margin: 100,
        mgn_mode: 'cross',
      },
    })

    expect(useTradingStore().positions).toEqual([{
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      pos_side: 'short',
      pos: -2,
      avg_px: 70000,
      mark_px: 71000.25,
      upl: -2000.5,
      upl_ratio: -0.0285,
      lever: 5,
      margin: 100,
      mgn_mode: 'cross',
    }])

    wrapper.unmount()
    await flushPromises()
    expect(unsubscribePositionsMock).toHaveBeenCalledWith('simulated')
  })

  it('忽略其他模式事件，pos 为 0 时删除对应持仓', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-position'])

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'live',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'short',
        pos: -1,
        mark_px: 71000,
      },
    })
    expect(useTradingStore().positions).toEqual([])

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'demo',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'short',
        pos: -1,
        mark_px: 71000,
      },
    })
    expect(useTradingStore().positions).toEqual([])

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'simulated',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'short',
        pos: -1,
        mark_px: 71000,
      },
    })
    expect(useTradingStore().positions).toHaveLength(1)

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'simulated',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'long',
        pos: 0,
        mark_px: 71100,
        raw: { posSide: 'net' },
      },
    })
    expect(useTradingStore().positions).toEqual([])

    wrapper.unmount()
    await flushPromises()
  })

  it('持仓数量未知时不把实时事件当作平仓删除', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-position'])

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'simulated',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'long',
        pos: 1,
        avg_px: 70000,
        mark_px: 71000,
      },
    })
    expect(useTradingStore().positions).toHaveLength(1)

    listeners['okxq-private-position']?.({
      payload: {
        mode: 'simulated',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'long',
        pos: null,
        avg_px: null,
        mark_px: null,
      },
    })

    expect(useTradingStore().positions).toEqual([{
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      pos_side: 'long',
      pos: 1,
      avg_px: 70000,
      mark_px: 71000,
      upl: null,
      upl_ratio: null,
      lever: null,
      margin: null,
      mgn_mode: 'cross',
    }])

    wrapper.unmount()
    await flushPromises()
  })

  it('账户权益和 USDT 明细未知时不显示为 0', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-account'])

    listeners['okxq-private-account']?.({
      payload: {
        mode: 'simulated',
        account: {
          total_eq: null,
          total_equity: null,
          iso_eq: null,
          adj_eq: null,
          raw: {
            totalEq: 'bad-total-equity',
            isoEq: 'bad-isolated-equity',
            adjEq: 'bad-adjusted-equity',
          },
        },
        data: {},
      },
    })

    expect(useTradingStore().account).toEqual({
      total_eq: null,
      iso_eq: null,
      adj_eq: null,
      usdt_balance: null,
      usdt_available: null,
      usdt_equity_usd: null,
      details: [],
    })

    wrapper.unmount()
    await flushPromises()
  })

  it('订阅 fills 并在卸载时释放', async () => {
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => subscribeFillsMock.mock.calls.length === 1)
    expect(subscribeFillsMock).toHaveBeenCalledWith('simulated')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeFillsMock).toHaveBeenCalledWith('simulated')
  })

  it('切换模式时释放旧模式私有订阅并订阅新模式', async () => {
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => subscribePositionsMock.mock.calls.length === 1)

    mode.value = 'live'
    await nextTick()
    await waitFor(() => subscribePositionsMock.mock.calls.length === 2)

    expect(unsubscribeAccountMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeOrdersMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeAlgoOrdersMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribeFillsMock).toHaveBeenCalledWith('simulated')
    expect(unsubscribePositionsMock).toHaveBeenCalledWith('simulated')
    expect(subscribeAccountMock).toHaveBeenCalledWith('live')
    expect(subscribeOrdersMock).toHaveBeenCalledWith('live')
    expect(subscribeAlgoOrdersMock).toHaveBeenCalledWith('live')
    expect(subscribeFillsMock).toHaveBeenCalledWith('live')
    expect(subscribePositionsMock).toHaveBeenCalledWith('live')

    wrapper.unmount()
    await flushPromises()

    expect(unsubscribeAccountMock).toHaveBeenCalledWith('live')
    expect(unsubscribeOrdersMock).toHaveBeenCalledWith('live')
    expect(unsubscribeAlgoOrdersMock).toHaveBeenCalledWith('live')
    expect(unsubscribeFillsMock).toHaveBeenCalledWith('live')
    expect(unsubscribePositionsMock).toHaveBeenCalledWith('live')
  })

  it('私有成交事件刷新成交列表', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-fill'])

    listeners['okxq-private-fill']?.({
      payload: {
        mode: 'simulated',
        trade_id: 'fill-1',
        ord_id: 'order-1',
        inst_id: 'BTC-USDT-SWAP',
        side: 'buy',
        fill_px: 70000,
        fill_sz: 0.25,
        fee: -0.01,
        fee_ccy: 'USDT',
        ts: 1_780_000_000_000,
      },
    })

    expect(useTradingStore().fills).toEqual([{
      fill_id: 'fill-1',
      ord_id: 'order-1',
      inst_id: 'BTC-USDT-SWAP',
      side: 'buy',
      fill_px: 70000,
      fill_sz: 0.25,
      fee: -0.01,
      fee_ccy: 'USDT',
      fill_time: 1_780_000_000_000,
    }])

    wrapper.unmount()
    await flushPromises()
  })

  it('忽略成交价格或数量无效的私有成交事件', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-fill'])

    listeners['okxq-private-fill']?.({
      payload: {
        mode: 'simulated',
        trade_id: 'fill-bad-price',
        ord_id: 'order-1',
        inst_id: 'BTC-USDT-SWAP',
        side: 'buy',
        fill_px: 0,
        fill_sz: 0.25,
        ts: 1_780_000_000_000,
      },
    })
    listeners['okxq-private-fill']?.({
      payload: {
        mode: 'simulated',
        trade_id: 'fill-bad-size',
        ord_id: 'order-1',
        inst_id: 'BTC-USDT-SWAP',
        side: 'sell',
        fill_px: 70000,
        fill_sz: 0,
        ts: 1_780_000_000_001,
      },
    })

    expect(useTradingStore().fills).toEqual([])

    wrapper.unmount()
    await flushPromises()
  })

  it('挂单数值无效时不显示为 0，终态事件仍删除旧挂单', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-order'])

    listeners['okxq-private-order']?.({
      payload: {
        mode: 'simulated',
        ord_id: 'order-live',
        inst_id: 'BTC-USDT-SWAP',
        side: 'buy',
        ord_type: 'limit',
        sz: 1,
        px: 70000,
        state: 'live',
        c_time: 1_780_000_000_000,
      },
    })
    expect(useTradingStore().orders).toHaveLength(1)

    listeners['okxq-private-order']?.({
      payload: {
        mode: 'simulated',
        ord_id: 'order-bad-size',
        inst_id: 'BTC-USDT-SWAP',
        side: 'buy',
        ord_type: 'limit',
        sz: 0,
        px: 70000,
        state: 'live',
        c_time: 1_780_000_000_001,
      },
    })
    listeners['okxq-private-order']?.({
      payload: {
        mode: 'simulated',
        ord_id: 'order-bad-price',
        inst_id: 'BTC-USDT-SWAP',
        side: 'sell',
        ord_type: 'limit',
        sz: 1,
        px: 0,
        state: 'live',
        c_time: 1_780_000_000_002,
      },
    })

    expect(useTradingStore().orders.map(order => order.ord_id)).toEqual(['order-live'])

    listeners['okxq-private-order']?.({
      payload: {
        mode: 'simulated',
        ord_id: 'order-live',
        inst_id: 'BTC-USDT-SWAP',
        side: 'buy',
        ord_type: 'limit',
        sz: null,
        px: null,
        state: 'canceled',
        c_time: 1_780_000_000_003,
      },
    })

    expect(useTradingStore().orders).toEqual([])

    wrapper.unmount()
    await flushPromises()
  })

  it('监听保护单事件并把仍生效的保护单放入挂单列表', async () => {
    const listeners = captureRealtimeListeners()
    const mode = ref<'simulated' | 'live'>('simulated')
    const wrapper = mountPrivateRealtime(mode)

    await waitFor(() => !!listeners['okxq-private-algo-order'])

    listeners['okxq-private-algo-order']?.({
      payload: {
        mode: 'simulated',
        algo_id: 'algo-live',
        inst_id: 'BTC-USDT-SWAP',
        side: 'sell',
        state: 'live',
        c_time: 1_780_000_000_010,
        raw: {
          sz: '2',
          slTriggerPx: '69000',
        },
      },
    })

    expect(useTradingStore().orders).toEqual([{
      ord_id: 'algo-live',
      inst_id: 'BTC-USDT-SWAP',
      side: 'sell',
      ord_type: 'market',
      sz: 2,
      px: 69000,
      state: 'live',
      fill_sz: null,
      fill_px: null,
      avg_px: null,
      pnl: null,
      ctime: 1_780_000_000_010,
    }])

    listeners['okxq-private-algo-order']?.({
      payload: {
        mode: 'simulated',
        algo_id: 'algo-live',
        inst_id: 'BTC-USDT-SWAP',
        side: 'sell',
        state: 'canceled',
      },
    })

    expect(useTradingStore().orders).toEqual([])

    wrapper.unmount()
    await flushPromises()
  })
})

function mountPrivateRealtime(mode: { value: 'simulated' | 'live' }) {
  return mount(defineComponent({
    setup() {
      usePrivateTradingRealtime(() => mode.value)
      return () => h('div')
    },
  }))
}

function captureRealtimeListeners() {
  const listeners: Record<string, RealtimeListener> = {}
  listenMock.mockImplementation(async (event, handler) => {
    listeners[String(event)] = handler as RealtimeListener
    return () => {}
  })
  return listeners
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 10; index += 1) {
    await flushPromises()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
