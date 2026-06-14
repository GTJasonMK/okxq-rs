import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import * as marketApi from '@/api/market'
import DataCenterView from '@/views/DataCenterView.vue'
import type {
  SyncJob,
  SyncRuntimeConfig,
  SyncRuntimeSettings,
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import type { MarketGapPlan } from '@/types/market'
import type {
  GuardianConfig,
  GuardianStatus,
  InventoryCacheRebuildProgress,
  InventoryRow,
  InventorySummary,
  TickCollectorStatus,
} from '@/types/dataCenter'

const routeState = vi.hoisted(() => ({
  path: '/data-center',
  query: {} as Record<string, unknown>,
}))
const routerPushMock = vi.hoisted(() => vi.fn())
const routerReplaceMock = vi.hoisted(() => vi.fn())

vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({
    push: routerPushMock,
    replace: routerReplaceMock,
  }),
}))

vi.mock('@/api/market', () => ({
  fetchWatchedSymbols: vi.fn(),
  fetchSyncJobs: vi.fn(),
  fetchSyncRecords: vi.fn(),
  fetchSyncRuntimeConfig: vi.fn(),
  fetchGuardianStatus: vi.fn(),
  fetchGuardianConfig: vi.fn(),
  fetchInventory: vi.fn(),
  rebuildInventoryCache: vi.fn(),
  startInventoryCacheRebuild: vi.fn(),
  fetchInventoryCacheRebuildStatus: vi.fn(),
  fetchMarketGapPlan: vi.fn(),
  startGapRepairJob: vi.fn(),
  fetchTickCollectorStatus: vi.fn(),
  runDataGuardianNow: vi.fn(),
  addWatchedSymbol: vi.fn(),
  updateSyncRuntimeConfig: vi.fn(),
  repairWatchedSymbol: vi.fn(),
  deleteWatchedSymbol: vi.fn(),
  cancelSyncJob: vi.fn(),
  startTickCollector: vi.fn(),
  stopTickCollector: vi.fn(),
}))

const fetchWatchedSymbolsMock = vi.mocked(marketApi.fetchWatchedSymbols)
const fetchSyncJobsMock = vi.mocked(marketApi.fetchSyncJobs)
const fetchSyncRecordsMock = vi.mocked(marketApi.fetchSyncRecords)
const fetchSyncRuntimeConfigMock = vi.mocked(marketApi.fetchSyncRuntimeConfig)
const fetchGuardianStatusMock = vi.mocked(marketApi.fetchGuardianStatus)
const fetchGuardianConfigMock = vi.mocked(marketApi.fetchGuardianConfig)
const fetchInventoryMock = vi.mocked(marketApi.fetchInventory)
const rebuildInventoryCacheMock = vi.mocked(marketApi.rebuildInventoryCache)
const startInventoryCacheRebuildMock = vi.mocked(marketApi.startInventoryCacheRebuild)
const fetchInventoryCacheRebuildStatusMock = vi.mocked(marketApi.fetchInventoryCacheRebuildStatus)
const fetchMarketGapPlanMock = vi.mocked(marketApi.fetchMarketGapPlan)
const startGapRepairJobMock = vi.mocked(marketApi.startGapRepairJob)
const fetchTickCollectorStatusMock = vi.mocked(marketApi.fetchTickCollectorStatus)
const addWatchedSymbolMock = vi.mocked(marketApi.addWatchedSymbol)
const repairWatchedSymbolMock = vi.mocked(marketApi.repairWatchedSymbol)

describe('DataCenterView 页面交互', () => {
  beforeEach(() => {
    window.localStorage.clear()
    routeState.path = '/data-center'
    routeState.query = {}
    routerPushMock.mockClear()
    routerReplaceMock.mockClear()
    fetchWatchedSymbolsMock.mockResolvedValue([
      watchedSymbol({ symbol: 'BTC-USDT', base_ccy: 'BTC' }),
    ])
    fetchSyncJobsMock
      .mockResolvedValueOnce([])
      .mockResolvedValue([syncJob({
        task_id: 'sync_gap_watchlist',
        status: 'completed',
        progress: 100,
        start_ts: 1777593600000,
        end_ts: 1777680000000,
        repair_method: 'paginated',
      })])
    fetchSyncRecordsMock.mockResolvedValue([])
    fetchSyncRuntimeConfigMock.mockResolvedValue(syncRuntimeConfig())
    fetchGuardianStatusMock.mockResolvedValue(guardianStatus())
    fetchGuardianConfigMock.mockResolvedValue(guardianConfig())
    fetchInventoryMock.mockResolvedValue({
      summary: inventorySummary(),
      rows: [inventoryRow()],
    })
    rebuildInventoryCacheMock.mockResolvedValue({
      message: '库存缓存已按 candles 全库扫描重建',
      candle_groups_scanned: 2,
      sync_records_rebuilt: 2,
      stale_sync_records_deleted: 1,
      sync_records_total: 2,
      cached_candles_total: 1234,
      inventory: {
        summary: inventorySummary({ symbol_count: 2 }),
        rows: [
          inventoryRow(),
          inventoryRow({ symbol: 'ETH-USDT', base_ccy: 'ETH' }),
        ],
      },
    })
    startInventoryCacheRebuildMock.mockResolvedValue({
      reused_existing: false,
      progress: inventoryRebuildProgress({ status: 'running', progress: 5 }),
    })
    fetchInventoryCacheRebuildStatusMock.mockResolvedValue({
      progress: inventoryRebuildProgress({ status: 'completed', progress: 100 }),
    })
    fetchMarketGapPlanMock.mockResolvedValue(marketGapPlan())
    startGapRepairJobMock.mockResolvedValue(syncJob())
    fetchTickCollectorStatusMock.mockResolvedValue(tickCollectorStatus())
    addWatchedSymbolMock.mockResolvedValue({
      watched_symbol: watchedSymbol({ symbol: 'ETH-USDT', base_ccy: 'ETH' }),
      existed: false,
      sync_jobs: [syncJob({
        task_id: 'sync_takeover_eth',
        inst_id: 'ETH-USDT-SWAP',
        status: 'queued',
        repair_method: 'auto',
      })],
      cancelled_disabled_jobs: [],
      started_count: 1,
      reused_count: 0,
      exact_gap_jobs: 1,
      rule_jobs: 0,
    })
    repairWatchedSymbolMock.mockResolvedValue({
      symbol: 'BTC-USDT',
      sync_jobs: [syncJob({
        task_id: 'sync_repair_rule',
        status: 'queued',
        repair_method: 'auto',
      })],
      requested_markets: { spot: false, swap: true },
      effective_markets: { spot: false, swap: true },
      started_count: 1,
      reused_count: 0,
      exact_gap_jobs: 1,
      rule_jobs: 0,
    })
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('路由 tab 驱动库存面板，库存行可跳转行情页', async () => {
    routeState.query = { tab: 'inventory', symbol: 'eth-usdt' }

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    expect(wrapper.find('.dc-tab.active').text()).toBe('数据库库存')
    expect(wrapper.text()).toContain('BTC-USDT')
    expect(wrapper.text()).toContain('1 周期')
    expect(wrapper.text()).toContain('无缺失')
    expect(routerReplaceMock).not.toHaveBeenCalled()

    const marketButton = wrapper.findAll('button').find(button => button.text() === '行情')
    expect(marketButton, 'expected inventory market button').toBeTruthy()
    await marketButton!.trigger('click')

    expect(routerPushMock).toHaveBeenCalledWith({
      path: '/market',
      query: { symbol: 'BTC-USDT' },
    })

    wrapper.unmount()
  })

  it('库存页可触发全库扫描并用重建结果刷新库存', async () => {
    routeState.query = { tab: 'inventory' }
    fetchInventoryMock
      .mockResolvedValueOnce({
        summary: inventorySummary(),
        rows: [inventoryRow()],
      })
      .mockResolvedValue({
        summary: inventorySummary({ symbol_count: 2 }),
        rows: [
          inventoryRow(),
          inventoryRow({ symbol: 'ETH-USDT', base_ccy: 'ETH' }),
        ],
      })

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    const rebuildButton = wrapper.findAll('button').find(button => button.text() === '全库扫描')
    expect(rebuildButton, 'expected inventory rebuild button').toBeTruthy()
    await rebuildButton!.trigger('click')
    await waitFor(() => startInventoryCacheRebuildMock.mock.calls.length > 0)
    await flushPromises()

    expect(startInventoryCacheRebuildMock).toHaveBeenCalledWith({ concurrency: 8 })
    expect(fetchInventoryCacheRebuildStatusMock).toHaveBeenCalled()
    expect(wrapper.text()).toContain('全库扫描完成：重建 2 条周期缓存')
    expect(wrapper.text()).toContain('清理陈旧缓存 1 条')
    expect(wrapper.text()).toContain('ETH-USDT')

    wrapper.unmount()
  })

  it('从库存切回关注币种时保留路由 symbol 并刷新关注数据', async () => {
    routeState.query = { tab: 'inventory', symbol: 'eth-usdt' }

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    const watchlistTab = wrapper.findAll('.dc-tab').find(tab => tab.text() === '数据标的')
    expect(watchlistTab, 'expected watchlist tab').toBeTruthy()
    await watchlistTab!.trigger('click')
    await waitFor(() => fetchWatchedSymbolsMock.mock.calls.length > 0)
    await flushPromises()

    expect((wrapper.find('.dc-input').element as HTMLInputElement).value).toBe('ETH-USDT')
    expect(routerPushMock).toHaveBeenCalledWith({
      path: '/data-center',
      query: { tab: 'watchlist', symbol: 'eth-usdt' },
    })
    expect(wrapper.text()).toContain('BTC-USDT')
    expect(wrapper.text()).toContain('1H · 30天 · 01/01 08:00 至 05/22 08:00 · 无缺失')

    wrapper.unmount()
  })

  it('关注列表成功时不因库存刷新失败显示为空关注', async () => {
    routeState.query = { tab: 'watchlist' }
    fetchInventoryMock.mockRejectedValueOnce(new Error('inventory unavailable'))

    const wrapper = mount(DataCenterView)
    await waitFor(() => (
      fetchWatchedSymbolsMock.mock.calls.length > 0 &&
      fetchInventoryMock.mock.calls.length > 0
    ))
    await flushPromises()

    expect(wrapper.text()).toContain('1 已接管规则')
    expect(wrapper.text()).toContain('BTC-USDT')
    expect(wrapper.text()).not.toContain('数据库暂无标的')
    expect(wrapper.text()).toContain('数据中心部分状态刷新失败：库存')

    wrapper.unmount()
  })

  it('关注数量不等待库存慢查询即可显示', async () => {
    routeState.query = { tab: 'watchlist' }
    let resolveInventory: (value: { summary: InventorySummary; rows: InventoryRow[] }) => void = () => {}
    const inventoryPromise = new Promise<{ summary: InventorySummary; rows: InventoryRow[] }>((resolve) => {
      resolveInventory = resolve
    })
    fetchInventoryMock.mockReturnValueOnce(inventoryPromise)

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchWatchedSymbolsMock.mock.calls.length > 0)
    await flushPromises()

    expect(wrapper.text()).toContain('1 已接管规则')
    expect(wrapper.text()).toContain('BTC-USDT')
    expect(wrapper.text()).toContain('刷新中')
    expect(wrapper.text()).not.toContain('数据库暂无标的')

    resolveInventory({
      summary: inventorySummary(),
      rows: [inventoryRow()],
    })
    await waitFor(() => !wrapper.text().includes('刷新中'))

    wrapper.unmount()
  })

  it('数据标的页展示数据库已有但未接管规则的标的', async () => {
    routeState.query = { tab: 'watchlist' }
    fetchInventoryMock.mockResolvedValueOnce({
      summary: inventorySummary({ symbol_count: 2, watched_symbol_count: 1 }),
      rows: [
        inventoryRow(),
        inventoryRow({
          symbol: 'ETH-USDT',
          base_ccy: 'ETH',
          managed: false,
          watched: false,
          markets: {
            SWAP: {
              ...inventoryRow().markets.SWAP!,
              inst_id: 'ETH-USDT-SWAP',
              managed: false,
              watched: false,
            },
          },
        }),
      ],
    })

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    expect(wrapper.text()).toContain('2 数据库标的')
    expect(wrapper.text()).toContain('1 已接管规则')
    expect(wrapper.text()).toContain('ETH-USDT')
    expect(wrapper.text()).toContain('库内未关注')
    expect(wrapper.text()).toContain('来源 数据库库存')
    expect(wrapper.findAll('button').some(button => button.text() === '接管规则')).toBe(true)

    wrapper.unmount()
  })

  it('库内已有标的接管规则后显示精确缺口同步结果', async () => {
    routeState.query = { tab: 'watchlist' }
    fetchInventoryMock.mockResolvedValueOnce({
      summary: inventorySummary({ symbol_count: 2, watched_symbol_count: 1 }),
      rows: [
        inventoryRow(),
        inventoryRow({
          symbol: 'ETH-USDT',
          base_ccy: 'ETH',
          managed: false,
          watched: false,
          markets: {
            SWAP: {
              ...inventoryRow().markets.SWAP!,
              inst_id: 'ETH-USDT-SWAP',
              managed: false,
              watched: false,
              timeframes: [
                {
                  ...inventoryRow().markets.SWAP!.timeframes[0],
                  timeframe: '1m',
                },
                {
                  ...inventoryRow().markets.SWAP!.timeframes[0],
                  timeframe: '3m',
                },
              ],
            },
          },
        }),
      ],
    })

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    const takeoverButton = wrapper.findAll('button').find(button => button.text() === '接管规则')
    expect(takeoverButton, 'expected inventory takeover button').toBeTruthy()
    await takeoverButton!.trigger('click')
    await flushPromises()

    const saveButton = wrapper.findAll('button').find(button => button.text() === '保存规则并同步')
    expect(saveButton, 'expected rule save button').toBeTruthy()
    await saveButton!.trigger('click')
    await waitFor(() => addWatchedSymbolMock.mock.calls.length > 0)
    await flushPromises()

    expect(addWatchedSymbolMock).toHaveBeenCalledWith('ETH-USDT', expect.objectContaining({
      sync_spot: false,
      sync_swap: true,
      auto_sync: true,
    }))
    const syncPlans = addWatchedSymbolMock.mock.calls[0][1]?.sync_plans?.filter(plan => plan.enabled)
    expect(syncPlans?.map(plan => plan.timeframe)).toEqual(['1m', '3m'])
    expect(wrapper.text()).toContain('ETH-USDT 已接管库内标的规则')
    expect(wrapper.text()).toContain('精确缺口 1 个')

    wrapper.unmount()
  })

  it('库存周期行可提交精确缺口补齐任务', async () => {
    routeState.query = { tab: 'inventory' }
    fetchInventoryMock.mockResolvedValue({
      summary: inventorySummary(),
      rows: [inventoryRowWithGap()],
    })
    fetchMarketGapPlanMock.mockResolvedValueOnce(marketGapPlan({
      missing_candles: 5,
      methods: {
        paginated_ranges: 1,
        historical_zip_ranges: 0,
      },
    }))
    startGapRepairJobMock.mockResolvedValueOnce(syncJob({
      task_id: 'sync_gap_001',
      status: 'queued',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      repair_method: 'paginated',
    }))
    fetchSyncJobsMock.mockResolvedValue([syncJob({
      task_id: 'sync_gap_001',
      status: 'completed',
      progress: 100,
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      repair_method: 'paginated',
    })])

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    const repairButton = wrapper.findAll('button').find(button => button.text() === '精确补齐')
    expect(repairButton, 'expected exact gap repair button').toBeTruthy()
    await repairButton!.trigger('click')
    await waitFor(() => startGapRepairJobMock.mock.calls.length > 0)
    await flushPromises()

    expect(fetchMarketGapPlanMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      limit: 100,
    })
    expect(startGapRepairJobMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      method: 'auto',
    })
    expect(wrapper.text()).toContain('已提交精确补齐')
    expect(wrapper.text()).toContain('缺失 5 根')

    wrapper.unmount()
  })

  it('关注币种缺失周期可提交精确补齐任务', async () => {
    fetchInventoryMock.mockResolvedValue({
      summary: inventorySummary(),
      rows: [inventoryRowWithGap()],
    })
    fetchMarketGapPlanMock.mockResolvedValueOnce(marketGapPlan({
      missing_candles: 5,
      methods: {
        paginated_ranges: 1,
        historical_zip_ranges: 0,
      },
    }))
    startGapRepairJobMock.mockResolvedValueOnce(syncJob({
      task_id: 'sync_gap_watchlist',
      status: 'queued',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      repair_method: 'paginated',
    }))
    fetchSyncJobsMock.mockResolvedValue([])

    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchInventoryMock.mock.calls.length > 0)
    await flushPromises()

    expect(wrapper.text()).toContain('缺失 5')
    const repairButton = wrapper.findAll('button').find(button => button.text() === '精确补齐')
    expect(repairButton, 'expected watchlist exact gap repair button').toBeTruthy()
    await repairButton!.trigger('click')
    await waitFor(() => startGapRepairJobMock.mock.calls.length > 0)
    await flushPromises()

    expect(fetchMarketGapPlanMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      limit: 100,
    })
    expect(startGapRepairJobMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      method: 'auto',
    })
    expect(wrapper.text()).toContain('已提交精确补齐')

    wrapper.unmount()
  })

  it('关注币种补齐按钮会展示按规则精确缺口任务数量', async () => {
    const wrapper = mount(DataCenterView)
    await waitFor(() => fetchWatchedSymbolsMock.mock.calls.length > 0)
    await flushPromises()

    const repairButton = wrapper.findAll('button').find(button => button.text() === '补齐')
    expect(repairButton, 'expected watched row repair button').toBeTruthy()
    await repairButton!.trigger('click')
    await waitFor(() => repairWatchedSymbolMock.mock.calls.length > 0)
    await flushPromises()

    expect(repairWatchedSymbolMock).toHaveBeenCalledWith('BTC-USDT', {
      sync_spot: false,
      sync_swap: true,
    })
    expect(wrapper.text()).toContain('精确缺口 1 个')

    wrapper.unmount()
  })
})

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
    ...overrides,
  }
}

function inventoryRebuildProgress(
  overrides: Partial<InventoryCacheRebuildProgress> = {},
): InventoryCacheRebuildProgress {
  return {
    task_id: 'inventory_rebuild_001',
    status: 'running',
    phase: 'scanning',
    progress: 25,
    message: '扫描 candles 索引中：500 / 1234 根',
    started_at: '2026-05-01T00:00:00.000000000+00:00',
    updated_at: '2026-05-01T00:00:01.000000000+00:00',
    finished_at: null,
    error: '',
    processed_candles: 500,
    target_candles: 1234,
    processed_groups: 1,
    target_groups: 3,
    scan_concurrency: 8,
    candle_groups_scanned: 1,
    sync_records_rebuilt: 2,
    stale_sync_records_deleted: 1,
    sync_records_total: 2,
    cached_candles_total: 1234,
    ...overrides,
  }
}

function marketGapPlan(overrides: Partial<MarketGapPlan> = {}): MarketGapPlan {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    source_timeframe: '1m',
    target_timeframes: ['1H'],
    range: {
      start_ts: 1777593600000,
      end_ts: 1777680000000,
      start_time: '2026-05-01T00:00:00.000Z',
      end_time: '2026-05-02T00:00:00.000Z',
    },
    local_range: {
      oldest_timestamp: 1777593600000,
      newest_timestamp: 1777680000000,
      oldest_time: '2026-05-01T00:00:00.000Z',
      newest_time: '2026-05-02T00:00:00.000Z',
    },
    expected_candles: 25,
    available_candles: 20,
    missing_candles: 5,
    coverage_ratio: 0.8,
    gap_event_count: 1,
    returned_gap_count: 1,
    returned_missing_candles: 5,
    truncated: false,
    max_internal_gap_ms: 3600000,
    methods: {
      paginated_ranges: 1,
      historical_zip_ranges: 0,
    },
    gaps: [],
    ...overrides,
  }
}

function syncRuntimeConfig(): SyncRuntimeConfig {
  const settings: SyncRuntimeSettings = {
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
  return {
    settings,
    defaults: { ...settings },
    limits: {},
    active_sync_jobs: 0,
  }
}

function guardianStatus(overrides: Partial<GuardianStatus> = {}): GuardianStatus {
  return {
    enabled: true,
    active: false,
    policy_summary: '1H rolling',
    rolling_window_timeframes: ['1H'],
    full_backfill_timeframes: [],
    watched_count: 1,
    backfill_queue_size: 0,
    current_inst_id: '',
    current_timeframe: '',
    current_mode: '',
    current_phase: '',
    last_successful_run_at: null,
    last_run_finished_at: null,
    backfill_queue_preview: [],
    last_sync_results: [],
    last_errors: [],
    ...overrides,
  }
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
    status: guardianStatus(),
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
    table_totals: { total: 1234, market_candles: 1234 },
    ...overrides,
  }
}

function inventoryRow(overrides: Partial<InventoryRow> = {}): InventoryRow {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    managed: true,
    watched: true,
    orphan: false,
    candle_count: 1234,
    timeframe_record_count: 1,
    storage_counts: { total: 1234, market_candles: 1234 },
    markets: {
      SWAP: {
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        managed: true,
        watched: true,
        timeframe_count: 1,
        candle_count: 1234,
        gap_count: 0,
        history_complete_count: 1,
        oldest_time: '2026-01-01T00:00:00.000000000+00:00',
        newest_time: '2026-05-22T00:00:00.000000000+00:00',
        last_sync_time: '2026-05-22T00:00:00.000000000+00:00',
        timeframes: [{
          timeframe: '1H',
          managed: true,
          candle_count: 1234,
          expected_candle_count: 1234,
          gap_count: 0,
          coverage_ratio: 1,
          history_complete: true,
          last_sync_mode: 'derive',
          last_sync_time: '2026-05-22T00:00:00.000000000+00:00',
          oldest_timestamp: 1767225600000,
          newest_timestamp: 1779408000000,
          oldest_time: '2026-01-01T00:00:00.000000000+00:00',
          newest_time: '2026-05-22T00:00:00.000000000+00:00',
        }],
      },
    },
    ...overrides,
  }
}

function inventoryRowWithGap(): InventoryRow {
  const row = inventoryRow()
  const market = row.markets.SWAP
  if (!market) return row
  return {
    ...row,
    markets: {
      ...row.markets,
      SWAP: {
        ...market,
        gap_count: 5,
        timeframes: [{
          timeframe: '1H',
          managed: true,
          candle_count: 20,
          expected_candle_count: 25,
          gap_count: 5,
          coverage_ratio: 0.8,
          history_complete: false,
          last_sync_mode: 'derive',
          last_sync_time: '2026-05-02T00:00:00.000000000+00:00',
          oldest_timestamp: 1777593600000,
          newest_timestamp: 1777680000000,
          oldest_time: '2026-05-01T00:00:00.000Z',
          newest_time: '2026-05-02T00:00:00.000Z',
        }],
      },
    },
  }
}

function tickCollectorStatus(overrides: Partial<TickCollectorStatus> = {}): TickCollectorStatus {
  return {
    running: false,
    active_symbols: [],
    book_channel: 'books5',
    total_trades_received: 0,
    total_bars_written: 0,
    last_trade_ts: 0,
    errors: [],
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
