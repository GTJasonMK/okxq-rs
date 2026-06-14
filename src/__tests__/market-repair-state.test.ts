import { computed } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useMarketRepairState } from '@/composables/marketViewState/repair'
import type { SyncJob, WatchedSymbol } from '@/types'

vi.mock('@/api/market', () => ({
  repairWatchedSymbol: vi.fn(),
  fetchSyncJobs: vi.fn(),
}))

const repairWatchedSymbolMock = vi.mocked(marketApi.repairWatchedSymbol)
const fetchSyncJobsMock = vi.mocked(marketApi.fetchSyncJobs)

describe('useMarketRepairState', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('行情页按关注规则补齐时分批观察任务，避免每轮提交全部 task_id', async () => {
    vi.useFakeTimers()
    const state = setupRepairState()
    const submitted = perfTaskIds(450).map(taskId => syncJob({
      task_id: taskId,
      status: 'queued',
    }))
    repairWatchedSymbolMock.mockResolvedValue({
      symbol: 'BTC-USDT',
      sync_jobs: submitted,
      requested_markets: { spot: false, swap: true },
      effective_markets: { spot: false, swap: true },
      started_count: submitted.length,
      reused_count: 0,
      exact_gap_jobs: submitted.length,
      rule_jobs: 0,
    })
    fetchSyncJobsMock.mockImplementation(async (params?: { task_ids?: string[] }) => (
      (params?.task_ids ?? []).map(taskId => syncJob({
        task_id: taskId,
        status: 'completed',
        progress: 100,
      }))
    ))

    const repair = state.repairActive()

    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 1)
    await vi.advanceTimersByTimeAsync(1200)
    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 2)
    await vi.advanceTimersByTimeAsync(1200)
    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 3)
    await repair

    const requestedBatches = fetchSyncJobsMock.mock.calls.map(([params]) => params?.task_ids ?? [])
    expect(requestedBatches.map(batch => batch.length)).toEqual([200, 200, 50])
    expect(requestedBatches[0][0]).toBe('market_repair_0000')
    expect(requestedBatches[1][0]).toBe('market_repair_0200')
    expect(requestedBatches[2][0]).toBe('market_repair_0400')
    expect(fetchSyncJobsMock.mock.calls.every(([, options]) => options?.dedupe === false)).toBe(true)
    expect(state.loadCandles).toHaveBeenCalledTimes(1)
    expect(state.loadMarketSnapshot).toHaveBeenCalledTimes(1)
    expect(state.repairing.value).toBe(false)
  })

})

function setupRepairState() {
  const loadCandles = vi.fn(async () => {})
  const loadMarketSnapshot = vi.fn(async () => {})
  const router = { push: vi.fn() }
  const store = { error: '' }
  const repairState = useMarketRepairState({
    activeBaseSymbol: computed(() => 'BTC-USDT'),
    activeWatchedSymbol: computed(() => watchedSymbol()),
    loadCandles,
    loadMarketSnapshot,
    router: router as never,
    store: store as never,
  })
  return {
    ...repairState,
    loadCandles,
    loadMarketSnapshot,
    router,
    store,
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

function perfTaskIds(count: number) {
  return Array.from({ length: count }, (_, index) => `market_repair_${index.toString().padStart(4, '0')}`)
}

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 20; index += 1) {
    await Promise.resolve()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}
