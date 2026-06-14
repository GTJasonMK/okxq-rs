import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, type Component } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import * as marketApi from '@/api/market'
import * as researchApi from '@/api/research'
import { useTrendResearchView } from '@/composables/useTrendResearchView'

vi.mock('@/api/market', () => ({
  fetchDefaultWatchScope: vi.fn(),
}))

vi.mock('@/api/research', () => ({
  fetchTrendConfig: vi.fn(),
  fetchTrendFactors: vi.fn(),
  computeFactors: vi.fn(),
  updateTrendConfig: vi.fn(),
}))

const fetchDefaultWatchScopeMock = vi.mocked(marketApi.fetchDefaultWatchScope)
const fetchTrendConfigMock = vi.mocked(researchApi.fetchTrendConfig)
const fetchTrendFactorsMock = vi.mocked(researchApi.fetchTrendFactors)
const computeFactorsMock = vi.mocked(researchApi.computeFactors)
const updateTrendConfigMock = vi.mocked(researchApi.updateTrendConfig)

describe('useTrendResearchView', () => {
  beforeEach(() => {
    fetchDefaultWatchScopeMock.mockResolvedValue({
      symbol: 'BTC-USDT',
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
    })
    fetchTrendConfigMock.mockResolvedValue({
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      bar_count: 500,
      enabled: false,
      whitelist: ['BTC-USDT-SWAP'],
    })
    fetchTrendFactorsMock.mockResolvedValue([])
    computeFactorsMock.mockResolvedValue([])
    updateTrendConfigMock.mockResolvedValue({})
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('保存非法 K 线数量后不会把 NaN 写入视图配置', async () => {
    const view = mountTrendResearchComposable()
    await flushPromises()

    await view.saveConfig({
      symbol: 'BTC-USDT',
      inst_type: 'SWAP',
      timeframe: '1H',
      bar_count: 'abc',
    })

    expect(Number.isFinite(view.config.value.bar_count)).toBe(true)
    expect(view.config.value.bar_count).toBe(500)
    expect(view.message.value).toBe('趋势研究配置已保存')
  })
})

function mountTrendResearchComposable() {
  let exposed!: ReturnType<typeof useTrendResearchView>
  const Harness: Component = defineComponent({
    setup() {
      exposed = useTrendResearchView()
      return () => h('div')
    },
  })
  mount(Harness)
  return exposed
}
