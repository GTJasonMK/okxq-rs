import { computed, defineComponent, reactive, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { useDataCenterShell } from '@/composables/useDataCenterShell'
import type { SyncJob } from '@/types'
import type { DataCenterTab, GuardianStatus } from '@/types/dataCenter'

describe('useDataCenterShell', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('挂载时应用路由 symbol 并同步首选 tab', () => {
    const state = setupShell({ query: { symbol: 'eth-usdt' }, preferredTab: 'inventory' })

    expect(state.symbolInput.value).toBe('ETH-USDT')
    expect(state.syncRouteTab).toHaveBeenCalledWith('inventory', true)

    state.wrapper.unmount()
  })

  it('打开行情页时按 symbol 跳转', () => {
    const state = setupShell()

    state.openMarket('BTC-USDT')

    expect(state.router.push).toHaveBeenCalledWith({
      path: '/market',
      query: { symbol: 'BTC-USDT' },
    })

    state.wrapper.unmount()
  })

  it('轮询时按当前 tab 刷新活跃同步任务来源', () => {
    const state = setupShell({
      activeTab: 'watchlist',
      jobs: [syncJob({ status: 'queued' })],
    })

    vi.advanceTimersByTime(3000)

    expect(state.refreshSyncProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshInventoryData).not.toHaveBeenCalled()
    expect(state.refreshGuardianStatus).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('关注列表高频轮询优先刷新同步任务进度，不重复刷新同步记录', () => {
    const refreshSyncJobProgressData = vi.fn(async () => {})
    const state = setupShell({
      activeTab: 'watchlist',
      jobs: [syncJob({ status: 'running' })],
      refreshSyncJobProgressData,
    })

    vi.advanceTimersByTime(3000)

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshSyncProgressData).not.toHaveBeenCalled()
    expect(state.refreshInventoryData).not.toHaveBeenCalled()
    expect(state.refreshGuardianStatus).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('任务进度轮询发现关注列表任务结束后刷新一次完整同步来源', async () => {
    let state!: ReturnType<typeof setupShell>
    const refreshSyncJobProgressData = vi.fn(async () => {
      state.jobs.value = []
    })
    state = setupShell({
      activeTab: 'watchlist',
      jobs: [syncJob({ status: 'running' })],
      refreshSyncJobProgressData,
    })

    state.pollActiveTabData()
    await flushPollRefresh()

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshSyncProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshInventoryData).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('没有活跃任务但仍在观察任务时刷新库存页', () => {
    const state = setupShell({
      activeTab: 'inventory',
      pendingObserve: true,
    })

    vi.advanceTimersByTime(3000)

    expect(state.refreshInventoryData).toHaveBeenCalledTimes(1)
    expect(state.refreshSyncProgressData).not.toHaveBeenCalled()
    expect(state.refreshGuardianStatus).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('库存页高频轮询只刷新任务进度，任务结束后再刷新库存', async () => {
    let state!: ReturnType<typeof setupShell>
    const refreshSyncJobProgressData = vi.fn(async () => {})
    state = setupShell({
      activeTab: 'inventory',
      jobs: [syncJob({ status: 'running' })],
      refreshSyncJobProgressData,
    })

    state.pollActiveTabData()
    await flushPollRefresh()

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshInventoryData).not.toHaveBeenCalled()

    state.jobs.value = []
    state.pollActiveTabData()
    await flushPollRefresh()

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(1)
    expect(state.refreshInventoryData).not.toHaveBeenCalled()

    state.jobs.value = [syncJob({ status: 'running', task_id: 'sync_inventory_terminal' })]
    refreshSyncJobProgressData.mockImplementationOnce(async () => {
      state.jobs.value = []
    })

    state.pollActiveTabData()
    await flushPollRefresh()

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(2)
    expect(state.refreshInventoryData).toHaveBeenCalledTimes(1)
    expect(state.refreshSyncProgressData).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('同步任务模块判定无需刷新时不触发当前 tab 全量刷新', () => {
    const state = setupShell({
      activeTab: 'watchlist',
      jobs: [syncJob({ status: 'running' })],
      shouldRefreshSyncJobSource: false,
    })

    vi.advanceTimersByTime(3000)

    expect(state.shouldRefreshSyncJobSource!).toHaveBeenCalledTimes(1)
    expect(state.refreshSyncProgressData).not.toHaveBeenCalled()
    expect(state.refreshInventoryData).not.toHaveBeenCalled()
    expect(state.refreshGuardianStatus).not.toHaveBeenCalled()

    state.wrapper.unmount()
  })

  it('Guardian 活跃时刷新 Guardian 状态，并在卸载后停止轮询', () => {
    const state = setupShell({
      activeTab: 'guardian',
      guardianStatus: { active: true },
    })

    vi.advanceTimersByTime(3000)
    expect(state.refreshGuardianStatus).toHaveBeenCalledTimes(1)

    state.wrapper.unmount()
    vi.advanceTimersByTime(3000)

    expect(state.refreshGuardianStatus).toHaveBeenCalledTimes(1)
  })

  it('同步进度上一轮刷新未完成时跳过同 tab 重复轮询', async () => {
    vi.useRealTimers()
    let resolveRefresh!: () => void
    const pendingRefresh = new Promise<void>((resolve) => {
      resolveRefresh = resolve
    })
    const refreshSyncProgressData = vi.fn(() => pendingRefresh)
    const state = setupShell({
      activeTab: 'watchlist',
      jobs: [syncJob({ status: 'running' })],
      refreshSyncProgressData,
    })

    state.pollActiveTabData()
    state.pollActiveTabData()
    state.pollActiveTabData()

    expect(refreshSyncProgressData).toHaveBeenCalledTimes(1)

    resolveRefresh()
    await Promise.resolve()
    await Promise.resolve()
    state.pollActiveTabData()

    expect(refreshSyncProgressData).toHaveBeenCalledTimes(2)
    state.wrapper.unmount()
  })

  it('库存页活跃任务轮询避免重复处理完整库存 payload', async () => {
    vi.useRealTimers()
    const polls = 3
    let pollIndex = 0
    let state!: ReturnType<typeof setupShell>
    const refreshSyncJobProgressData = vi.fn(async () => {
      pollIndex += 1
      if (pollIndex === polls) state.jobs.value = []
    })
    const refreshInventoryData = vi.fn(async () => {})
    state = setupShell({
      activeTab: 'inventory',
      jobs: [syncJob({ status: 'running' })],
      refreshSyncJobProgressData,
      refreshInventoryData,
    })

    for (let index = 0; index < polls; index += 1) {
      state.pollActiveTabData()
      await flushPollRefresh()
    }

    expect(refreshSyncJobProgressData).toHaveBeenCalledTimes(polls)
    expect(refreshInventoryData).toHaveBeenCalledTimes(1)

    state.wrapper.unmount()
  })
})

function setupShell(overrides: {
  activeTab?: DataCenterTab
  preferredTab?: DataCenterTab
  query?: Record<string, string>
  jobs?: SyncJob[]
  guardianStatus?: GuardianStatus | null
  pendingObserve?: boolean
  shouldRefreshSyncJobSource?: boolean
  refreshSyncProgressData?: () => Promise<void>
  refreshSyncJobProgressData?: () => Promise<void>
  refreshInventoryData?: () => Promise<void>
  refreshGuardianStatus?: () => Promise<void>
} = {}) {
  const route = reactive({
    query: overrides.query ?? {},
  })
  const router = {
    push: vi.fn(),
  }
  const symbolInput = ref('')
  const activeTab = ref<DataCenterTab>(overrides.activeTab ?? 'watchlist')
  const jobs = ref<SyncJob[]>(overrides.jobs ?? [])
  const guardianStatus = ref<GuardianStatus | null>(overrides.guardianStatus ?? null)
  const resolvePreferredTab = vi.fn(() => overrides.preferredTab ?? activeTab.value)
  const syncRouteTab = vi.fn()
  const hasPendingSyncJobObserve = vi.fn(() => Boolean(overrides.pendingObserve))
  const shouldRefreshSyncJobSource = overrides.shouldRefreshSyncJobSource === undefined
    ? undefined
    : vi.fn(() => overrides.shouldRefreshSyncJobSource as boolean)
  const refreshSyncProgressData = vi.fn(overrides.refreshSyncProgressData ?? (async () => {}))
  const refreshSyncJobProgressData = overrides.refreshSyncJobProgressData
    ? vi.fn(overrides.refreshSyncJobProgressData)
    : undefined
  const refreshInventoryData = vi.fn(overrides.refreshInventoryData ?? (async () => {}))
  const refreshGuardianStatus = vi.fn(overrides.refreshGuardianStatus ?? (async () => {}))

  let shell!: ReturnType<typeof useDataCenterShell>
  const component = defineComponent({
    setup() {
      shell = useDataCenterShell({
        route,
        router,
        symbolInput,
        activeTab,
        activeJobs: computed(() => jobs.value),
        guardianStatus,
        resolvePreferredTab,
        syncRouteTab,
        hasPendingSyncJobObserve,
        shouldRefreshSyncJobSource,
        refreshSyncProgressData,
        refreshSyncJobProgressData,
        refreshInventoryData,
        refreshGuardianStatus,
      })
      return shell
    },
    template: '<div />',
  })

  const wrapper = mount(component)

  return {
    wrapper,
    router,
    symbolInput,
    openMarket: shell.openMarket,
    pollActiveTabData: shell.pollActiveTabData,
    jobs,
    syncRouteTab,
    shouldRefreshSyncJobSource,
    refreshSyncProgressData,
    refreshSyncJobProgressData,
    refreshInventoryData,
    refreshGuardianStatus,
  }
}

async function flushPollRefresh() {
  await Promise.resolve()
  await Promise.resolve()
}

function syncJob(overrides: Partial<SyncJob> = {}): SyncJob {
  return {
    task_id: 'sync_gap_001',
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    source_timeframe: '1m',
    target_timeframes: ['1H'],
    mode: 'gap_repair',
    status: 'queued',
    progress: 0,
    start_ts: 1000,
    end_ts: 2000,
    repair_method: 'auto',
    created_at: '2026-05-01T00:00:00.000000000+00:00',
    updated_at: '2026-05-01T00:00:00.000000000+00:00',
    finished_at: null,
    ...overrides,
  }
}
