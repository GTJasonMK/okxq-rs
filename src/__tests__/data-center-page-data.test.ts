import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterPageData } from '@/composables/useDataCenterPageData'
import type { SyncJob, SyncRecord, SyncRuntimeConfig, SyncRuntimeSettings, WatchedSymbol } from '@/types'
import type { GuardianConfig, InventoryRow, InventorySummary } from '@/types/dataCenter'

vi.mock('@/api/market', () => ({
  fetchWatchedSymbols: vi.fn(),
  fetchInventory: vi.fn(),
  fetchSyncRecords: vi.fn(),
  fetchSyncJobs: vi.fn(),
  fetchGuardianConfig: vi.fn(),
  fetchSyncRuntimeConfig: vi.fn(),
}))

const fetchWatchedSymbolsMock = vi.mocked(marketApi.fetchWatchedSymbols)
const fetchInventoryMock = vi.mocked(marketApi.fetchInventory)
const fetchSyncRecordsMock = vi.mocked(marketApi.fetchSyncRecords)
const fetchSyncJobsMock = vi.mocked(marketApi.fetchSyncJobs)
const fetchGuardianConfigMock = vi.mocked(marketApi.fetchGuardianConfig)
const fetchSyncRuntimeConfigMock = vi.mocked(marketApi.fetchSyncRuntimeConfig)

describe('useDataCenterPageData', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('聚合加载时写入关注列表，并分发库存、任务、Guardian 和采集配置', async () => {
    const state = setupPageData()
    const inventory = inventoryPayload()
    const records = [syncRecord()]
    const jobs = [syncJob()]
    const config = guardianConfig()
    const runtimeConfig = syncRuntimeConfig()
    state.applyInventoryPayload.mockReturnValue(records)
    fetchWatchedSymbolsMock.mockResolvedValue([watchedSymbol()])
    fetchInventoryMock.mockResolvedValue(inventory)
    fetchSyncJobsMock.mockResolvedValue(jobs)
    fetchGuardianConfigMock.mockResolvedValue(config)
    fetchSyncRuntimeConfigMock.mockResolvedValue(runtimeConfig)

    await state.pageData.loadPageData()

    expect(fetchSyncJobsMock).toHaveBeenCalledWith({ limit: 200, watched_only: true })
    expect(state.pageData.watchedSymbols.value).toEqual([watchedSymbol()])
    expect(state.applyInventoryPayload).toHaveBeenCalledWith(inventory)
    expect(state.applyFetchedSyncJobs).toHaveBeenCalledWith(jobs, records)
    expect(state.applyGuardianConfig).toHaveBeenCalledWith(config, [])
    expect(state.syncRuntimeConfig.value).toStrictEqual(runtimeConfig)
    expect(state.error.value).toBe('')
    expect(state.pageData.loading.value).toBe(false)
  })

  it('库存刷新失败时仍保留关注列表并用现有记录分发任务', async () => {
    const existingRecords = [syncRecord({ timeframe: '5m' })]
    const state = setupPageData({ syncRecords: existingRecords })
    const jobs = [syncJob()]
    fetchWatchedSymbolsMock.mockResolvedValue([watchedSymbol()])
    fetchInventoryMock.mockRejectedValue(new Error('inventory unavailable'))
    fetchSyncJobsMock.mockResolvedValue(jobs)
    fetchGuardianConfigMock.mockResolvedValue(guardianConfig())
    fetchSyncRuntimeConfigMock.mockResolvedValue(syncRuntimeConfig())

    await state.pageData.loadPageData()

    expect(state.pageData.watchedSymbols.value).toEqual([watchedSymbol()])
    expect(state.applyInventoryPayload).not.toHaveBeenCalled()
    expect(state.applyFetchedSyncJobs).toHaveBeenCalledWith(jobs, existingRecords)
    expect(state.error.value).toContain('数据中心部分状态刷新失败：库存')
    expect(state.pageData.loading.value).toBe(false)
  })

  it('刷新同步进度时只更新同步记录和任务状态', async () => {
    const state = setupPageData()
    const records = [syncRecord()]
    const jobs = [syncJob({ task_id: 'sync_progress_1' })]
    fetchSyncRecordsMock.mockResolvedValue(records)
    fetchSyncJobsMock.mockResolvedValue(jobs)

    await state.pageData.refreshSyncProgressData()

    expect(fetchSyncRecordsMock).toHaveBeenCalledWith({ watched_only: true })
    expect(fetchInventoryMock).not.toHaveBeenCalled()
    expect(fetchSyncJobsMock).toHaveBeenCalledWith({ limit: 200, watched_only: true })
    expect(state.applyInventoryPayload).not.toHaveBeenCalled()
    expect(state.replaceSyncRecordScopes).toHaveBeenCalledWith(records, new Set())
    expect(state.syncRecords.value).toEqual(records)
    expect(state.applyFetchedSyncJobs).toHaveBeenCalledWith(jobs, records)
    expect(fetchWatchedSymbolsMock).not.toHaveBeenCalled()
    expect(fetchGuardianConfigMock).not.toHaveBeenCalled()
    expect(fetchSyncRuntimeConfigMock).not.toHaveBeenCalled()
  })

  it('轮询同步任务进度时复用现有记录，不重复拉取同步记录', async () => {
    const existingRecords = [syncRecord({ timeframe: '5m' })]
    const state = setupPageData({ syncRecords: existingRecords })
    const jobs = [syncJob({ task_id: 'sync_progress_jobs_only', status: 'running' })]
    fetchSyncJobsMock.mockResolvedValue(jobs)

    await state.pageData.refreshSyncJobProgressData()

    expect(fetchSyncJobsMock).toHaveBeenCalledWith({ limit: 200, watched_only: true })
    expect(fetchSyncRecordsMock).not.toHaveBeenCalled()
    expect(state.replaceSyncRecordScopes).not.toHaveBeenCalled()
    expect(state.applyFetchedSyncJobs).toHaveBeenCalledWith(jobs, existingRecords)
  })

  it('轻量任务轮询不会取消正在返回的完整同步记录刷新', async () => {
    const state = setupPageData()
    const recordsRequest = deferred<SyncRecord[]>()
    const fullJobsRequest = deferred<SyncJob[]>()
    const lightJobsRequest = deferred<SyncJob[]>()
    const records = [syncRecord({ timeframe: '1m' })]
    const fullJobs = [syncJob({ task_id: 'sync_progress_full', status: 'running', progress: 10 })]
    const lightJobs = [syncJob({ task_id: 'sync_progress_light', status: 'running', progress: 20 })]
    fetchSyncRecordsMock.mockReturnValueOnce(recordsRequest.promise)
    fetchSyncJobsMock
      .mockReturnValueOnce(fullJobsRequest.promise)
      .mockReturnValueOnce(lightJobsRequest.promise)

    const fullRefresh = state.pageData.refreshSyncProgressData()
    const lightRefresh = state.pageData.refreshSyncJobProgressData()
    lightJobsRequest.resolve(lightJobs)
    await lightRefresh

    recordsRequest.resolve(records)
    fullJobsRequest.resolve(fullJobs)
    await fullRefresh

    expect(state.applyFetchedSyncJobs).toHaveBeenCalledWith(lightJobs, [])
    expect(state.replaceSyncRecordScopes).toHaveBeenCalledWith(records, new Set())
    expect(state.syncRecords.value).toEqual(records)
    expect(state.applyFetchedSyncJobs).not.toHaveBeenCalledWith(fullJobs, records)
  })

  it('高频同步任务轮询跳过低频同步记录替换', async () => {
    const records = [syncRecord()]
    const jobs = [syncJob({ task_id: 'sync_progress_fast', status: 'running', progress: 30 })]
    const polls = 3
    const state = setupPageData({ syncRecords: records })

    fetchSyncJobsMock.mockResolvedValue(jobs)

    for (let index = 0; index < polls; index += 1) {
      await state.pageData.refreshSyncJobProgressData()
    }

    expect(fetchSyncRecordsMock).not.toHaveBeenCalled()
    expect(state.replaceSyncRecordScopes).not.toHaveBeenCalled()
    expect(fetchSyncJobsMock).toHaveBeenCalledTimes(polls)
    expect(state.applyFetchedSyncJobs).toHaveBeenCalledTimes(polls)
    expect(state.applyFetchedSyncJobs).toHaveBeenLastCalledWith(jobs, records)
  })

  it('刷新同步进度时只替换关注范围记录并保留库存孤立记录', async () => {
    const oldWatchedRecord = syncRecord({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1m',
      candle_count: 10,
    })
    const inventoryOnlyRecord = syncRecord({
      inst_id: 'DOGE-USDT',
      inst_type: 'SPOT',
      timeframe: '5m',
      candle_count: 20,
    })
    const refreshedWatchedRecord = syncRecord({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      candle_count: 30,
    })
    const state = setupPageData({
      syncRecords: [oldWatchedRecord, inventoryOnlyRecord],
    })
    state.replaceSyncRecordScopes.mockImplementation((records: SyncRecord[]): SyncRecord[] => {
      state.syncRecords.value = records
      return [inventoryOnlyRecord, ...records]
    })
    const jobs = [syncJob({ task_id: 'sync_progress_2' })]
    fetchWatchedSymbolsMock.mockResolvedValue([watchedSymbol()])
    fetchInventoryMock.mockResolvedValue(inventoryPayload())
    fetchSyncJobsMock.mockResolvedValue([])
    fetchGuardianConfigMock.mockResolvedValue(guardianConfig())
    fetchSyncRuntimeConfigMock.mockResolvedValue(syncRuntimeConfig())

    await state.pageData.loadPageData()

    fetchSyncRecordsMock.mockResolvedValue([refreshedWatchedRecord])
    fetchSyncJobsMock.mockResolvedValue(jobs)

    await state.pageData.refreshSyncProgressData()

    expect(state.syncRecords.value).toEqual([
      refreshedWatchedRecord,
    ])
    expect(state.replaceSyncRecordScopes).toHaveBeenLastCalledWith(
      [refreshedWatchedRecord],
      new Set(['SWAP:BTC-USDT-SWAP']),
    )
    expect(state.applyFetchedSyncJobs).toHaveBeenLastCalledWith(jobs, [
      inventoryOnlyRecord,
      refreshedWatchedRecord,
    ])
  })
})

function setupPageData(overrides: { syncRecords?: SyncRecord[] } = {}) {
  const syncRecords = ref<SyncRecord[]>(overrides.syncRecords ?? [])
  const syncRuntimeConfig = ref<SyncRuntimeConfig | null>(null)
  const error = ref('')
  const applyInventoryPayload = vi.fn(
    (_inventory: { summary: InventorySummary; rows: InventoryRow[] }): SyncRecord[] => [],
  )
  const replaceSyncRecordScopes = vi.fn((records: SyncRecord[], _scopeKeys: Set<string>): SyncRecord[] => {
    syncRecords.value = records
    return records
  })
  const applyFetchedSyncJobs = vi.fn((jobs: SyncJob[]): SyncJob[] => jobs)
  const applyGuardianConfig = vi.fn((config: GuardianConfig) => config.settings.plans)
  const pageData = useDataCenterPageData({
    syncRecords,
    syncRuntimeConfig,
    applyInventoryPayload,
    replaceSyncRecordScopes,
    applyFetchedSyncJobs,
    applyGuardianConfig,
    error,
  })

  return {
    pageData,
    syncRecords,
    syncRuntimeConfig,
    error,
    applyInventoryPayload,
    replaceSyncRecordScopes,
    applyFetchedSyncJobs,
    applyGuardianConfig,
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
    sync_plans: [],
    created_at: '2026-05-22T00:00:00.000000000+00:00',
    updated_at: '2026-05-22T00:00:00.000000000+00:00',
    ...overrides,
  }
}

function inventoryPayload(overrides: Partial<{ summary: InventorySummary; rows: InventoryRow[] }> = {}) {
  return {
    summary: inventorySummary(),
    rows: [],
    ...overrides,
  }
}

function inventorySummary(overrides: Partial<InventorySummary> = {}): InventorySummary {
  return {
    symbol_count: 1,
    managed_symbol_count: 1,
    managed_market_count: 1,
    watched_symbol_count: 1,
    watched_list_count: 1,
    watched_market_count: 1,
    orphan_symbol_count: 0,
    total_candles: 1234,
    total_timeframe_records: 1,
    table_totals: { total: 1234 },
    ...overrides,
  }
}

function syncRecord(overrides: Partial<SyncRecord> = {}): SyncRecord {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    last_sync_time: '2026-05-01T00:00:00.000000000+00:00',
    candle_count: 10,
    history_complete: true,
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
    start_ts: 1000,
    end_ts: 2000,
    repair_method: 'auto',
    created_at: '2026-05-01T00:00:00.000000000+00:00',
    updated_at: '2026-05-01T00:00:00.000000000+00:00',
    finished_at: null,
    ...overrides,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve
    reject = promiseReject
  })
  return { promise, resolve, reject }
}

function guardianConfig(): GuardianConfig {
  const settings = {
    enabled: true,
    scan_interval_seconds: 3600,
    max_full_backfill_jobs_per_cycle: 1,
    plans: [{
      timeframe: '1H',
      enabled: true,
      bootstrap_days: 30,
      archive_mode: 'rolling',
    }],
  }
  return {
    settings,
    defaults: { ...settings, plans: [...settings.plans] },
  }
}

function runtimeSettings(): SyncRuntimeSettings {
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
  }
}

function syncRuntimeConfig(): SyncRuntimeConfig {
  const settings = runtimeSettings()
  return {
    settings,
    defaults: { ...settings },
    limits: {},
    active_sync_jobs: 0,
  }
}
