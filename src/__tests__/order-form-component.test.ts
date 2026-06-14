import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { invoke } from '@tauri-apps/api/core'
import { createPinia, setActivePinia } from 'pinia'
import OrderForm from '@/components/trading/OrderForm.vue'
import { useSystemStore } from '@/stores/systemStore'

const invokeMock = vi.mocked(invoke)
let pinia: ReturnType<typeof createPinia>

describe('OrderForm 组件', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    useSystemStore().applySystemStatus({ okx: { mode: 'simulated' } })
    invokeMock.mockClear()
    invokeMock.mockImplementation(mockInvoke)
  })

  it('合约快捷按钮通过拆分后的面板生成对应下单 payload', async () => {
    const wrapper = mount(OrderForm, {
      props: {
        mode: 'simulated',
      },
      global: {
        plugins: [pinia],
      },
    })
    await settle()

    await wrapper.findAll('.intent-btn')
      .find(button => button.text() === '开空')
      ?.trigger('click')
    await wrapper.findAll('.type-btn')
      .find(button => button.text() === '市价')
      ?.trigger('click')
    await wrapper.get('.submit-btn').trigger('click')
    await settle()

    expect(orderRequestBody()).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      side: 'sell',
      ord_type: 'market',
      pos_side: 'short',
      reduce_only: false,
      mode: 'simulated',
    })
  })
})

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await flushPromises()
  }
}

function orderRequestBody() {
  return requestBody('/api/trading/order')
}

function requestBody(path: string) {
  const call = invokeMock.mock.calls.find(([, args]) => {
    const req = requestFrom(args)
    return req?.method === 'POST' && req.path === path
  })
  if (!call) throw new Error(`missing request ${path}`)
  return requestFrom(call[1])?.body as Record<string, unknown>
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
    return ok({ posMode: 'long_short_mode' })
  }
  if (path.startsWith('/api/trading/contract/leverage/')) {
    return ok([{
      instId: 'BTC-USDT-SWAP',
      mgnMode: 'cross',
      posSide: 'long',
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
