import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope, nextTick, reactive, type EffectScope } from 'vue'
import type { LocationQuery, LocationQueryRaw } from 'vue-router'
import { useDataCenterTabs } from '@/composables/useDataCenterTabs'
import type { DataCenterTab } from '@/types/dataCenter'
import { DATA_CENTER_TAB_KEY, DATA_CENTER_TABS } from '@/utils/dataCenter'

const activeScopes: EffectScope[] = []

describe('useDataCenterTabs', () => {
  beforeEach(() => {
    window.localStorage.clear()
  })

  afterEach(() => {
    activeScopes.splice(0).forEach(scope => scope.stop())
    vi.clearAllMocks()
    window.localStorage.clear()
  })

  it('优先使用路由 tab 并生成当前 tab 提示', () => {
    window.localStorage.setItem(DATA_CENTER_TAB_KEY, 'inventory')

    const tabs = setupTabs({ tab: 'guardian' })

    expect(tabs.activeTab.value).toBe('guardian')
    expect(tabs.activeTabHint.value).toBe('查看后台守护器策略、队列和最近扫描结果')
    expect(tabs.resolvePreferredTab()).toBe('guardian')
    expect(tabs.loadTabData).not.toHaveBeenCalled()
  })

  it('首次同步用 replace 补齐偏好 tab 并保留其他 query', async () => {
    window.localStorage.setItem(DATA_CENTER_TAB_KEY, 'inventory')
    const tabs = setupTabs({ symbol: 'eth-usdt' })

    await tabs.syncRouteTab(tabs.resolvePreferredTab(), true)

    expect(tabs.activeTab.value).toBe('inventory')
    expect(tabs.loadTabData).toHaveBeenCalledWith('inventory')
    expect(window.localStorage.getItem(DATA_CENTER_TAB_KEY)).toBe('inventory')
    expect(tabs.router.replace).toHaveBeenCalledWith({
      path: '/data-center',
      query: { symbol: 'eth-usdt', tab: 'inventory' },
    })
    expect(tabs.router.push).not.toHaveBeenCalled()
  })

  it('用户切换 tab 用 push 同步路由并保留 symbol', async () => {
    const tabs = setupTabs({ tab: 'inventory', symbol: 'eth-usdt' })

    await tabs.setActiveTab('watchlist')

    expect(tabs.activeTab.value).toBe('watchlist')
    expect(tabs.loadTabData).toHaveBeenCalledWith('watchlist')
    expect(window.localStorage.getItem(DATA_CENTER_TAB_KEY)).toBe('watchlist')
    expect(tabs.router.push).toHaveBeenCalledWith({
      path: '/data-center',
      query: { tab: 'watchlist', symbol: 'eth-usdt' },
    })
    expect(tabs.router.replace).not.toHaveBeenCalled()
  })

  it('监听路由 tab 变化并刷新对应数据', async () => {
    const tabs = setupTabs({ tab: 'watchlist' })

    tabs.route.query = { tab: 'collection' }
    await nextTick()

    expect(tabs.activeTab.value).toBe('collection')
    expect(tabs.loadTabData).toHaveBeenCalledWith('collection')
    expect(window.localStorage.getItem(DATA_CENTER_TAB_KEY)).toBe('collection')
  })
})

function setupTabs(initialQuery: LocationQuery) {
  const route = reactive({
    path: '/data-center',
    query: { ...initialQuery } as LocationQuery,
  })
  const router = {
    push: vi.fn(async (_location: { path: string; query: LocationQueryRaw }) => {}),
    replace: vi.fn(async (_location: { path: string; query: LocationQueryRaw }) => {}),
  }
  const loadTabData = vi.fn(async (_tab: DataCenterTab) => {})

  let tabs!: ReturnType<typeof useDataCenterTabs>
  const scope = effectScope()
  scope.run(() => {
    tabs = useDataCenterTabs({
      tabs: DATA_CENTER_TABS,
      route,
      router,
      loadTabData,
    })
  })
  activeScopes.push(scope)

  return {
    ...tabs,
    route,
    router,
    loadTabData,
  }
}
