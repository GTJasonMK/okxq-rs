import { computed, ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterWatchlistActions } from '@/composables/useDataCenterWatchlistActions'
import type {
  SyncJob,
  SyncRuntimeConfig,
  SyncRuntimeSettings,
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import type { WatchedRow } from '@/types/dataCenter'
import { summarizeSyncProgress } from '@/utils/syncProgress'

vi.mock('@/api/market', () => ({
  addWatchedSymbol: vi.fn(),
  updateSyncRuntimeConfig: vi.fn(),
  repairWatchedSymbol: vi.fn(),
  cancelSyncJob: vi.fn(),
  deleteWatchedSymbol: vi.fn(),
}))

const addWatchedSymbolMock = vi.mocked(marketApi.addWatchedSymbol)
const updateSyncRuntimeConfigMock = vi.mocked(marketApi.updateSyncRuntimeConfig)
const repairWatchedSymbolMock = vi.mocked(marketApi.repairWatchedSymbol)
const cancelSyncJobMock = vi.mocked(marketApi.cancelSyncJob)
const deleteWatchedSymbolMock = vi.mocked(marketApi.deleteWatchedSymbol)

describe('useDataCenterWatchlistActions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('保存关注规则时同步采集参数、提交规则、跟踪任务并刷新页面', async () => {
    const state = setupActions({
      watchedRows: [watchedRow({ symbol: 'ETH-USDT', inventory_only: true })],
      rule: {
        newSymbol: 'eth-usdt',
        pendingRuleSymbol: 'ETH-USDT',
        syncSpot: false,
        syncSwap: true,
        autoSync: true,
        syncPlans: [syncPlan({ timeframe: '1m' }), syncPlan({ timeframe: '3m' })],
      },
    })
    const currentConfig = syncRuntimeConfig(runtimeSettings({ max_sync_batches: 3 }))
    const nextSettings = runtimeSettings({ max_sync_batches: 5 })
    state.actions.syncRuntimeConfig.value = currentConfig
    updateSyncRuntimeConfigMock.mockResolvedValue(syncRuntimeConfig(nextSettings))
    addWatchedSymbolMock.mockResolvedValue({
      watched_symbol: watchedSymbol({ symbol: 'ETH-USDT' }),
      existed: false,
      sync_jobs: [syncJob({ task_id: 'sync_takeover_eth' })],
      cancelled_disabled_jobs: [],
      started_count: 1,
      reused_count: 0,
      exact_gap_jobs: 1,
      rule_jobs: 0,
    })

    await state.actions.addSymbol(nextSettings)

    expect(updateSyncRuntimeConfigMock).toHaveBeenCalledWith(nextSettings)
    expect(addWatchedSymbolMock).toHaveBeenCalledWith('ETH-USDT', {
      sync_spot: false,
      sync_swap: true,
      archive_all_history: false,
      sync_days: 30,
      sync_plans: expect.arrayContaining([
        expect.objectContaining({ timeframe: '1m', enabled: true }),
        expect.objectContaining({ timeframe: '3m', enabled: true }),
      ]),
      auto_sync: true,
    })
    expect(state.rule.resetAfterSaved).toHaveBeenCalledTimes(1)
    expect(state.trackSubmittedJobs).toHaveBeenCalledWith([
      expect.objectContaining({ task_id: 'sync_takeover_eth' }),
    ])
    expect(state.loadPageData).toHaveBeenCalledTimes(1)
    expect(state.message.value).toContain('ETH-USDT 已接管库内标的规则')
    expect(state.message.value).toContain('精确缺口 1 个')
    expect(state.adding.value).toBe(false)
    expect(state.actions.savingSyncRuntime.value).toBe(false)
  })

  it('手动保存采集参数时写入配置和提示', async () => {
    const state = setupActions()
    const settings = runtimeSettings({ max_sync_batches: 7 })
    updateSyncRuntimeConfigMock.mockResolvedValue(syncRuntimeConfig(settings))

    await state.actions.saveSyncRuntimeConfig(settings)

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(updateSyncRuntimeConfigMock).toHaveBeenCalledWith(settings)
    expect(state.actions.syncRuntimeConfig.value?.settings.max_sync_batches).toBe(7)
    expect(state.message.value).toBe('数据采集性能参数已保存，后续新建同步任务生效')
    expect(state.actions.savingSyncRuntime.value).toBe(false)
  })

  it('按关注规则补齐时提交任务、跟踪结果并刷新页面', async () => {
    const state = setupActions()
    repairWatchedSymbolMock.mockResolvedValue({
      symbol: 'BTC-USDT',
      sync_jobs: [syncJob({ task_id: 'sync_repair_btc' })],
      requested_markets: { spot: false, swap: true },
      effective_markets: { spot: false, swap: true },
      started_count: 1,
      reused_count: 0,
      exact_gap_jobs: 1,
      rule_jobs: 0,
    })

    await state.actions.repairSymbol(watchedSymbol({ sync_spot: false, sync_swap: true }))

    expect(repairWatchedSymbolMock).toHaveBeenCalledWith('BTC-USDT', {
      sync_spot: false,
      sync_swap: true,
    })
    expect(state.trackSubmittedJobs).toHaveBeenCalledWith([
      expect.objectContaining({ task_id: 'sync_repair_btc' }),
    ])
    expect(state.loadPageData).toHaveBeenCalledTimes(1)
    expect(state.message.value).toContain('BTC-USDT 已按关注规则提交补齐')
    expect(state.actions.repairingSymbol.value).toBe('')
  })

  it('取消行内活跃任务时只取消 queued/running 任务', async () => {
    const state = setupActions()
    cancelSyncJobMock.mockResolvedValue(undefined)

    await state.actions.cancelRowActiveJobs(watchedRow({
      jobs: [
        syncJob({ task_id: 'queued_1', status: 'queued' }),
        syncJob({ task_id: 'running_1', status: 'running' }),
        syncJob({ task_id: 'completed_1', status: 'completed' }),
      ],
    }))

    expect(cancelSyncJobMock).toHaveBeenCalledTimes(2)
    expect(cancelSyncJobMock).toHaveBeenCalledWith('queued_1')
    expect(cancelSyncJobMock).toHaveBeenCalledWith('running_1')
    expect(state.message.value).toBe('BTC-USDT 已取消 2 个运行中的同步任务')
    expect(state.loadPageData).toHaveBeenCalledTimes(1)
  })

  it('删除关注标的后刷新页面并复位删除状态', async () => {
    const state = setupActions()
    deleteWatchedSymbolMock.mockResolvedValue(undefined)

    await state.actions.deleteSymbol('BTC-USDT')

    expect(deleteWatchedSymbolMock).toHaveBeenCalledWith('BTC-USDT')
    expect(state.message.value).toBe('BTC-USDT 已移出关注清单，相关本地数据和运行中任务已清理')
    expect(state.loadPageData).toHaveBeenCalledTimes(1)
    expect(state.actions.deletingSymbol.value).toBe('')
  })
})

function setupActions(overrides: {
  watchedRows?: WatchedRow[]
  rule?: Partial<{
    newSymbol: string
    pendingRuleSymbol: string
    syncSpot: boolean
    syncSwap: boolean
    archiveAll: boolean
    autoSync: boolean
    syncDays: number
    syncPlans: WatchedSymbolSyncPlan[]
    canSubmit: boolean
  }>
} = {}) {
  const message = ref('')
  const error = ref('')
  const adding = ref(false)
  const syncRuntimeConfig = ref<SyncRuntimeConfig | null>(null)
  const watchedRows = ref<WatchedRow[]>(overrides.watchedRows ?? [])
  const rule = {
    newSymbol: ref(overrides.rule?.newSymbol ?? 'BTC-USDT'),
    pendingRuleSymbol: ref(overrides.rule?.pendingRuleSymbol ?? 'BTC-USDT'),
    syncSpot: ref(overrides.rule?.syncSpot ?? false),
    syncSwap: ref(overrides.rule?.syncSwap ?? true),
    archiveAll: ref(overrides.rule?.archiveAll ?? false),
    autoSync: ref(overrides.rule?.autoSync ?? true),
    syncDays: ref(overrides.rule?.syncDays ?? 30),
    syncPlans: ref<WatchedSymbolSyncPlan[]>(overrides.rule?.syncPlans ?? [syncPlan()]),
    canSubmit: computed(() => overrides.rule?.canSubmit ?? true),
    resetAfterSaved: vi.fn(),
  }
  const clearFeedback = vi.fn(() => {
    message.value = ''
    error.value = ''
  })
  const loadPageData = vi.fn(async () => {})
  const trackSubmittedJobs = vi.fn()
  const actions = useDataCenterWatchlistActions({
    message,
    error,
    adding,
    syncRuntimeConfig,
    watchedRows: computed(() => watchedRows.value),
    rule,
    clearFeedback,
    loadPageData,
    trackSubmittedJobs,
  })

  return {
    actions,
    message,
    error,
    adding,
    rule,
    clearFeedback,
    loadPageData,
    trackSubmittedJobs,
  }
}

function watchedRow(overrides: Partial<WatchedRow> = {}): WatchedRow {
  const jobs = overrides.jobs ?? []
  return {
    ...watchedSymbol(),
    jobs,
    jobSummary: summarizeSyncProgress(jobs),
    ...overrides,
  }
}

function watchedSymbol(overrides: Partial<WatchedSymbol> = {}): WatchedSymbol {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    spot_inst_id: 'BTC-USDT',
    swap_inst_id: 'BTC-USDT-SWAP',
    sync_spot: false,
    sync_swap: true,
    archive_all_history: false,
    sync_days: 30,
    sync_plans: [syncPlan()],
    created_at: '2026-05-22T00:00:00.000000000+00:00',
    updated_at: '2026-05-22T00:00:00.000000000+00:00',
    ...overrides,
  }
}

function syncPlan(overrides: Partial<WatchedSymbolSyncPlan> = {}): WatchedSymbolSyncPlan {
  return {
    timeframe: '1H',
    enabled: true,
    bootstrap_days: 30,
    archive_mode: 'rolling',
    ...overrides,
  }
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
    start_ts: 1777593600000,
    end_ts: 1777680000000,
    repair_method: 'auto',
    created_at: '2026-05-01T00:00:00.000000000+00:00',
    updated_at: '2026-05-01T00:00:00.000000000+00:00',
    finished_at: null,
    ...overrides,
  }
}

function runtimeSettings(overrides: Partial<SyncRuntimeSettings> = {}): SyncRuntimeSettings {
  return {
    max_sync_batches: 3,
    okx_page_pause_ms: 120,
    sync_job_concurrency: 2,
    window_fetch_concurrency: 2,
    window_fetch_batches_per_slice: 2,
    candle_upsert_transaction_chunk: 1000,
    okx_max_concurrency: 4,
    okx_public_rest_concurrency: 4,
    okx_private_rest_concurrency: 2,
    okx_trade_rest_concurrency: 2,
    okx_ws_control_concurrency: 2,
    okx_unknown_concurrency: 1,
    ...overrides,
  }
}

function syncRuntimeConfig(settings: SyncRuntimeSettings = runtimeSettings()): SyncRuntimeConfig {
  return {
    settings,
    defaults: { ...settings },
    limits: {},
    active_sync_jobs: 0,
  }
}
