import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterOperations } from '@/composables/useDataCenterOperations'
import type { SyncJob } from '@/types'
import type { DataCenterTab, InventoryGapRepairPayload } from '@/types/dataCenter'
import type { MarketGapPlan } from '@/types/market'

vi.mock('@/api/market', () => ({
  fetchMarketGapPlan: vi.fn(),
  startGapRepairJob: vi.fn(),
  runDataGuardianNow: vi.fn(),
}))

const fetchMarketGapPlanMock = vi.mocked(marketApi.fetchMarketGapPlan)
const startGapRepairJobMock = vi.mocked(marketApi.startGapRepairJob)
const runDataGuardianNowMock = vi.mocked(marketApi.runDataGuardianNow)

describe('useDataCenterOperations', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('缺口补齐范围无效时直接暴露错误，不调用接口', async () => {
    const state = setupOperations()

    await state.operations.repairInventoryGap(gapPayload({ end_ts: 999 }))

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(state.error.value).toBe('BTC-USDT-SWAP 1H 缺少有效本地时间范围，无法精确补齐')
    expect(fetchMarketGapPlanMock).not.toHaveBeenCalled()
    expect(startGapRepairJobMock).not.toHaveBeenCalled()
  })

  it('缺口计划没有缺失时只刷新当前数据源', async () => {
    const state = setupOperations()
    fetchMarketGapPlanMock.mockResolvedValue(marketGapPlan({ missing_candles: 0 }))

    await state.operations.repairInventoryGap(gapPayload())

    expect(fetchMarketGapPlanMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1000,
      end_ts: 2000,
      limit: 100,
    })
    expect(startGapRepairJobMock).not.toHaveBeenCalled()
    expect(state.message.value).toBe('BTC-USDT-SWAP 1H 当前范围无缺失 K 线')
    expect(state.refreshActiveGapRepairSource).toHaveBeenCalledTimes(1)
    expect(state.operations.gapRepairingKey.value).toBe('')
  })

  it('缺口计划有缺失时提交精确补齐任务并跟踪结果', async () => {
    const state = setupOperations()
    fetchMarketGapPlanMock.mockResolvedValue(marketGapPlan({
      range: {
        start_ts: 1500,
        end_ts: 2500,
        start_time: '2026-05-01T00:00:00.000Z',
        end_time: '2026-05-02T00:00:00.000Z',
      },
      missing_candles: 5,
      methods: {
        paginated_ranges: 1,
        historical_zip_ranges: 0,
      },
    }))
    startGapRepairJobMock.mockResolvedValue(syncJob({ task_id: 'sync_gap_001' }))

    await state.operations.repairInventoryGap(gapPayload())

    expect(startGapRepairJobMock).toHaveBeenCalledWith({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1500,
      end_ts: 2500,
      method: 'auto',
    })
    expect(state.message.value).toContain('BTC-USDT-SWAP 1H 已提交精确补齐')
    expect(state.message.value).toContain('缺失 5 根')
    expect(state.trackSubmittedJobs).toHaveBeenCalledWith([
      expect.objectContaining({ task_id: 'sync_gap_001' }),
    ])
    expect(state.refreshActiveGapRepairSource).toHaveBeenCalledTimes(1)
    expect(state.operations.gapRepairingKey.value).toBe('')
  })

  it('Guardian 手动运行在 Guardian 页刷新 Guardian 数据', async () => {
    const state = setupOperations({ activeTab: 'guardian' })
    runDataGuardianNowMock.mockResolvedValue({
      last_sync_results: [syncJob({ task_id: 'sync_guardian_1' })],
    })

    await state.operations.runGuardian()

    expect(runDataGuardianNowMock).toHaveBeenCalledTimes(1)
    expect(state.message.value).toBe('已按当前关注清单和每币种采集规则提交补齐扫描')
    expect(state.trackSubmittedJobs).toHaveBeenCalledWith([
      expect.objectContaining({ task_id: 'sync_guardian_1' }),
    ])
    expect(state.loadGuardianData).toHaveBeenCalledTimes(1)
    expect(state.loadPageData).not.toHaveBeenCalled()
    expect(state.operations.guardianRunning.value).toBe(false)
  })

  it('Guardian 手动运行在非 Guardian 页刷新页面数据', async () => {
    const state = setupOperations({ activeTab: 'watchlist' })
    runDataGuardianNowMock.mockResolvedValue({ last_sync_results: [] })

    await state.operations.runGuardian()

    expect(state.loadPageData).toHaveBeenCalledTimes(1)
    expect(state.loadGuardianData).not.toHaveBeenCalled()
    expect(state.operations.guardianRunning.value).toBe(false)
  })
})

function setupOperations(overrides: { activeTab?: DataCenterTab } = {}) {
  const activeTab = ref<DataCenterTab>(overrides.activeTab ?? 'inventory')
  const message = ref('')
  const error = ref('')
  const clearFeedback = vi.fn(() => {
    message.value = ''
    error.value = ''
  })
  const refreshActiveGapRepairSource = vi.fn(async () => {})
  const loadPageData = vi.fn(async () => {})
  const loadGuardianData = vi.fn(async () => {})
  const trackSubmittedJobs = vi.fn()
  const operations = useDataCenterOperations({
    activeTab,
    message,
    error,
    clearFeedback,
    refreshActiveGapRepairSource,
    loadPageData,
    loadGuardianData,
    trackSubmittedJobs,
  })

  return {
    operations,
    activeTab,
    message,
    error,
    clearFeedback,
    refreshActiveGapRepairSource,
    loadPageData,
    loadGuardianData,
    trackSubmittedJobs,
  }
}

function gapPayload(overrides: Partial<InventoryGapRepairPayload> = {}): InventoryGapRepairPayload {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    start_ts: 1000,
    end_ts: 2000,
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
      start_ts: 1000,
      end_ts: 2000,
      start_time: '2026-05-01T00:00:00.000Z',
      end_time: '2026-05-02T00:00:00.000Z',
    },
    local_range: {
      oldest_timestamp: 1000,
      newest_timestamp: 2000,
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
