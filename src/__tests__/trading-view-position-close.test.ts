import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'
import * as tradingApi from '@/api/trading'
import PositionTable from '@/components/trading/PositionTable.vue'
import { useTradingView } from '@/composables/useTradingView'
import { useSystemStore } from '@/stores/systemStore'
import type { AccountInfo, Position } from '@/types'

const routeState = vi.hoisted(() => ({
  query: {} as Record<string, unknown>,
}))

vi.mock('vue-router', () => ({
  useRoute: () => routeState,
}))

vi.mock('@/composables/usePrivateTradingRealtime', async () => {
  const { ref } = await vi.importActual<typeof import('vue')>('vue')
  return {
    usePrivateTradingRealtime: () => ({
      connected: ref(false),
      error: ref(null),
      connectedMode: ref('simulated'),
    }),
  }
})

vi.mock('@/api/trading', async () => {
  const actual = await vi.importActual<typeof import('@/api/trading')>('@/api/trading')
  return {
    ...actual,
    fetchAccount: vi.fn(),
    fetchPositions: vi.fn(),
    fetchOrders: vi.fn(),
    fetchFills: vi.fn(),
    placeOrder: vi.fn(),
  }
})

const fetchAccountMock = vi.mocked(tradingApi.fetchAccount)
const fetchPositionsMock = vi.mocked(tradingApi.fetchPositions)
const fetchOrdersMock = vi.mocked(tradingApi.fetchOrders)
const fetchFillsMock = vi.mocked(tradingApi.fetchFills)
const placeOrderMock = vi.mocked(tradingApi.placeOrder)

let pinia: ReturnType<typeof createPinia>

describe('交易页持仓操作', () => {
  beforeEach(() => {
    routeState.query = {}
    pinia = createPinia()
    setActivePinia(pinia)
    useSystemStore().applySystemStatus({ okx: { mode: 'simulated' } })
    fetchAccountMock.mockResolvedValue(account())
    fetchPositionsMock.mockResolvedValue([])
    fetchOrdersMock.mockResolvedValue([])
    fetchFillsMock.mockResolvedValue([])
    placeOrderMock.mockResolvedValue({ ok: true })
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('对已有空头持仓提交市价 reduce-only 平空订单', async () => {
    const { view, wrapper } = mountTradingView()
    await settle()

    await view.handleClosePosition(position({
      pos_side: 'short',
      pos: -2,
      mgn_mode: 'isolated',
    }))
    await settle()

    expect(placeOrderMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      td_mode: 'isolated',
      side: 'buy',
      ord_type: 'market',
      sz: 2,
      pos_side: 'short',
      reduce_only: true,
      mode: 'simulated',
    })
    expect(view.message.value).toContain('平空市价单已提交')
    wrapper.unmount()
  })

  it('持仓表平仓按钮需要二次确认后才发出 close 事件', async () => {
    const wrapper = mount(PositionTable, {
      props: {
        positions: [position({ pos_side: 'long', pos: 2 })],
        modeLocked: false,
        closingPositionKeys: new Set<string>(),
      },
    })

    await wrapper.find('.position-action.danger').trigger('click')

    expect(wrapper.emitted('close')).toBeUndefined()
    const confirm = wrapper.findAll('button').find(button => button.text() === '确认')
    expect(confirm).toBeTruthy()

    await confirm!.trigger('click')

    expect(wrapper.emitted('close')?.[0]?.[0]).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      pos_side: 'long',
      pos: 2,
    })
    wrapper.unmount()
  })
})

function mountTradingView() {
  let view!: ReturnType<typeof useTradingView>
  const wrapper = mount(defineComponent({
    setup() {
      view = useTradingView()
      return () => h('div')
    },
  }), {
    global: { plugins: [pinia] },
  })
  return { view, wrapper }
}

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await flushPromises()
  }
}

function account(): AccountInfo {
  return {
    total_eq: 1000,
    iso_eq: 0,
    adj_eq: 1000,
    usdt_balance: 1000,
    usdt_available: 1000,
    usdt_equity_usd: 1000,
    details: [],
  }
}

function position(overrides: Partial<Position> = {}): Position {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    pos_side: 'long',
    pos: 1,
    mgn_mode: 'cross',
    avg_px: 70000,
    upl: 0,
    upl_ratio: 0,
    lever: 3,
    margin: 100,
    mark_px: 70000,
    ...overrides,
  }
}
