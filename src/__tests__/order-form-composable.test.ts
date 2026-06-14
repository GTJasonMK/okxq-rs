import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'
import { useOrderForm } from '@/composables/useOrderForm'
import { useSystemStore } from '@/stores/systemStore'

const invokeMock = vi.mocked(invoke)

let accountPosMode: 'net_mode' | 'long_short_mode' = 'net_mode'
let pinia: ReturnType<typeof createPinia>

describe('下单表单合约参数', () => {
  beforeEach(() => {
    accountPosMode = 'net_mode'
    pinia = createPinia()
    setActivePinia(pinia)
    useSystemStore().applySystemStatus({ okx: { mode: 'simulated' } })
    invokeMock.mockImplementation(mockInvoke)
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('单向持仓合约下单不提交 pos_side', async () => {
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    orderForm.form.ord_type = 'market'
    orderForm.form.side = 'sell'
    orderForm.form.sz = 1
    orderForm.form.sync_leverage = false

    await orderForm.submit()
    await settle()

    const body = orderRequestBody()
    expect(body).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      td_mode: 'cross',
      side: 'sell',
      ord_type: 'market',
      sz: '1',
      mode: 'simulated',
    })
    expect(body).not.toHaveProperty('pos_side')
    wrapper.unmount()
  })

  it('双向持仓合约下单提交 long/short pos_side', async () => {
    accountPosMode = 'long_short_mode'
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    orderForm.form.ord_type = 'market'
    orderForm.form.side = 'sell'
    orderForm.form.pos_side = 'short'
    orderForm.form.sz = 1
    orderForm.form.sync_leverage = false

    await orderForm.submit()
    await settle()

    expect(orderRequestBody()).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      td_mode: 'cross',
      side: 'sell',
      ord_type: 'market',
      pos_side: 'short',
      sz: '1',
      mode: 'simulated',
    })
    wrapper.unmount()
  })

  it('双向持仓快捷操作能生成开空和平空请求', async () => {
    accountPosMode = 'long_short_mode'
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    orderForm.form.ord_type = 'market'
    orderForm.form.sz = 1
    orderForm.form.sync_leverage = false
    orderForm.applyContractIntent('open_short')

    await orderForm.submit()
    await settle()

    expect(orderRequestBody()).toMatchObject({
      side: 'sell',
      pos_side: 'short',
      reduce_only: false,
    })

    invokeMock.mockClear()
    orderForm.applyContractIntent('close_short')

    await orderForm.submit()
    await settle()

    expect(orderRequestBody()).toMatchObject({
      side: 'buy',
      pos_side: 'short',
      reduce_only: true,
    })
    wrapper.unmount()
  })

  it('全仓同步杠杆不提交 pos_side，逐仓双向才提交 pos_side', async () => {
    accountPosMode = 'long_short_mode'
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    await orderForm.applyLeverage()
    await settle()

    expect(leverageRequestBody()).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      lever: '3',
      mgn_mode: 'cross',
      mode: 'simulated',
    })
    expect(leverageRequestBody()).not.toHaveProperty('pos_side')

    invokeMock.mockClear()
    orderForm.form.td_mode = 'isolated'
    orderForm.form.pos_side = 'short'

    await orderForm.applyLeverage()
    await settle()

    expect(leverageRequestBody()).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      lever: '3',
      mgn_mode: 'isolated',
      pos_side: 'short',
      mode: 'simulated',
    })
    wrapper.unmount()
  })

  it('下单前同步杠杆不阻塞刷新合约元数据', async () => {
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    invokeMock.mockClear()
    orderForm.form.ord_type = 'market'
    orderForm.form.side = 'sell'
    orderForm.form.sz = 1
    orderForm.form.sync_leverage = true

    await orderForm.submit()
    await settle()

    expect(requestPaths()).toEqual([
      '/api/trading/contract/set-leverage',
      '/api/trading/order',
    ])
    wrapper.unmount()
  })

  it('关注品种作为主入口，手动 instId 默认收起并可按需展开', async () => {
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    expect(orderForm.showManualSymbolInput.value).toBe(false)
    expect(orderForm.orderInstId.value).toBe('BTC-USDT-SWAP')

    orderForm.showManualSymbolEditor()
    expect(orderForm.showManualSymbolInput.value).toBe(true)

    orderForm.manualSymbolModel.value = 'ETH-USDT'
    expect(orderForm.form.inst_id).toBe('ETH-USDT')
    expect(orderForm.orderInstId.value).toBe('ETH-USDT-SWAP')
    expect(orderForm.showManualSymbolInput.value).toBe(true)

    wrapper.unmount()
  })

  it('合约隐藏买入卖出切换，现货才显示买入卖出', async () => {
    const { orderForm, wrapper } = mountOrderForm()
    await settle()

    expect(orderForm.showSideToggle.value).toBe(false)
    expect(orderForm.contractIntentOptions.value.map(option => option.label)).toEqual(['做多', '做空'])

    orderForm.marketTypeModel.value = 'SPOT'

    expect(orderForm.showSideToggle.value).toBe(true)
    expect(orderForm.sideOptions.value.map(option => option.label)).toEqual(['买入', '卖出'])
    expect(orderForm.orderInstId.value).toBe('BTC-USDT')

    wrapper.unmount()
  })
})

function mountOrderForm() {
  let orderForm!: ReturnType<typeof useOrderForm>
  const Host = defineComponent({
    setup() {
      orderForm = useOrderForm({ mode: () => 'simulated' })
      return () => h('div')
    },
  })
  const wrapper = mount(Host, {
    global: { plugins: [pinia] },
  })
  return { orderForm, wrapper }
}

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await flushPromises()
  }
}

function orderRequestBody() {
  return requestBody('/api/trading/order')
}

function leverageRequestBody() {
  return requestBody('/api/trading/contract/set-leverage')
}

function requestBody(path: string) {
  const call = invokeMock.mock.calls.find(([, args]) => {
    const req = requestFrom(args)
    return req?.method === 'POST' && req.path === path
  })
  if (!call) throw new Error(`missing request ${path}`)
  return requestFrom(call[1])?.body as Record<string, unknown>
}

function requestPaths() {
  return invokeMock.mock.calls
    .map(([, args]) => requestFrom(args)?.path ?? '')
    .filter(Boolean)
}

function mockInvoke(command: string, args?: unknown) {
  if (command !== 'local_api_request') return Promise.resolve({})
  const req = requestFrom(args)
  const path = req?.path ?? ''
  if (path === '/api/market/watched-symbols') {
    return ok([{
      symbol: 'BTC-USDT',
      base_ccy: 'BTC',
      spot_inst_id: 'BTC-USDT',
      swap_inst_id: 'BTC-USDT-SWAP',
      sync_spot: true,
      sync_swap: true,
    }])
  }
  if (path === '/api/trading/contract/account-config') {
    return ok({ posMode: accountPosMode })
  }
  if (path.startsWith('/api/trading/contract/leverage/')) {
    return ok([{
      instId: 'BTC-USDT-SWAP',
      mgnMode: 'cross',
      posSide: accountPosMode === 'long_short_mode' ? 'long' : '',
      lever: '3',
    }])
  }
  if (path === '/api/trading/order') return ok({ ok: true })
  if (path === '/api/trading/contract/set-leverage') return ok({ ok: true })
  return ok({})
}

function ok(data: unknown) {
  return Promise.resolve({ code: 0, data })
}

function requestFrom(value: unknown) {
  if (!isRecord(value) || !isRecord(value.req)) return null
  return value.req as {
    method?: string
    path?: string
    params?: Record<string, unknown>
    body?: unknown
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
