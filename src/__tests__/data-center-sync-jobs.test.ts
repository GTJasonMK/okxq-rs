import { ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterSyncJobs } from '@/composables/useDataCenterSyncJobs'
import type { SyncJob, SyncRecord } from '@/types'

vi.mock('@/api/market', () => ({
  fetchSyncJobs: vi.fn(),
}))

const fetchSyncJobsMock = vi.mocked(marketApi.fetchSyncJobs)

describe('useDataCenterSyncJobs', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('应用拉取结果时只保留仍可见的任务并计算活跃任务', () => {
    const syncJobs = setupSyncJobs()

    syncJobs.applyFetchedSyncJobs([
      syncJob({ task_id: 'queued_1', status: 'queued' }),
      syncJob({
        task_id: 'completed_old',
        status: 'completed',
        created_at: '1970-01-01T00:00:00.000Z',
        updated_at: '1970-01-01T00:00:00.000Z',
        finished_at: '1970-01-01T00:00:00.000Z',
      }),
    ])

    expect(syncJobs.syncJobs.value.map(job => job.task_id)).toEqual(['queued_1'])
    expect(syncJobs.activeJobs.value.map(job => job.task_id)).toEqual(['queued_1'])
    expect(syncJobs.shouldRefreshSyncJobSource()).toBe(true)
  })

  it('跟踪新提交任务，观察接口返回终态时只刷新一次来源', async () => {
    const syncJobs = setupSyncJobs()
    fetchSyncJobsMock.mockResolvedValueOnce([
      syncJob({
        task_id: 'sync_submitted',
        status: 'completed',
        progress: 100,
      }),
    ])

    syncJobs.trackSubmittedJobs([
      syncJob({
        task_id: 'sync_submitted',
        status: 'queued',
      }),
    ])

    expect(syncJobs.syncJobs.value).toMatchObject([
      { task_id: 'sync_submitted', status: 'queued' },
    ])
    expect(syncJobs.activeJobs.value).toHaveLength(1)
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(true)
    expect(syncJobs.shouldRefreshSyncJobSource()).toBe(false)

    await waitFor(() => fetchSyncJobsMock.mock.calls.length > 0)
    await waitFor(() => syncJobs.refreshObservedSource.mock.calls.length === 1)

    expect(fetchSyncJobsMock).toHaveBeenCalledWith({
      task_ids: ['sync_submitted'],
      limit: 1,
    }, { dedupe: false })
    expect(syncJobs.refreshObservedSource).toHaveBeenCalledTimes(1)
    expect(syncJobs.syncJobs.value).toMatchObject([
      { task_id: 'sync_submitted', status: 'completed' },
    ])
    expect(syncJobs.activeJobs.value).toHaveLength(0)
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(false)
    expect(syncJobs.shouldRefreshSyncJobSource()).toBe(false)
  })

  it('观察中的任务运行轮询不触发全量来源刷新，终态后再刷新一次', async () => {
    vi.useFakeTimers()
    const syncJobs = setupSyncJobs()
    fetchSyncJobsMock
      .mockResolvedValueOnce([
        syncJob({
          task_id: 'sync_submitted',
          status: 'running',
          progress: 45,
        }),
      ])
      .mockResolvedValueOnce([
        syncJob({
          task_id: 'sync_submitted',
          status: 'completed',
          progress: 100,
        }),
      ])

    syncJobs.trackSubmittedJobs([
      syncJob({
        task_id: 'sync_submitted',
        status: 'queued',
      }),
    ])

    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 1)

    expect(syncJobs.syncJobs.value).toMatchObject([
      { task_id: 'sync_submitted', status: 'running', progress: 45 },
    ])
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(true)
    expect(syncJobs.shouldRefreshSyncJobSource()).toBe(false)
    expect(syncJobs.refreshObservedSource).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1200)
    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 2)
    await waitFor(() => syncJobs.refreshObservedSource.mock.calls.length === 1)

    expect(syncJobs.syncJobs.value).toMatchObject([
      { task_id: 'sync_submitted', status: 'completed', progress: 100 },
    ])
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(false)
    expect(syncJobs.shouldRefreshSyncJobSource()).toBe(false)
    expect(syncJobs.refreshObservedSource).toHaveBeenCalledTimes(1)
  })

  it('大批量提交任务按后端上限分批观察，避免单轮提交全部 pending task_id', async () => {
    vi.useFakeTimers()
    const syncJobs = setupSyncJobs()
    const submitted = perfTaskIds(450).map(taskId => syncJob({
      task_id: taskId,
      status: 'queued',
    }))
    fetchSyncJobsMock.mockImplementation(async (params?: { task_ids?: string[] }) => (
      (params?.task_ids ?? []).map(taskId => syncJob({
        task_id: taskId,
        status: 'completed',
        progress: 100,
      }))
    ))

    syncJobs.trackSubmittedJobs(submitted)

    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 1)
    await vi.advanceTimersByTimeAsync(1200)
    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 2)
    await vi.advanceTimersByTimeAsync(1200)
    await waitFor(() => fetchSyncJobsMock.mock.calls.length === 3)
    await waitFor(() => syncJobs.refreshObservedSource.mock.calls.length === 1)

    const requestedBatches = fetchSyncJobsMock.mock.calls.map(([params]) => params?.task_ids ?? [])
    expect(requestedBatches.map(batch => batch.length)).toEqual([200, 200, 50])
    expect(requestedBatches[0][0]).toBe('sync_bulk_0000')
    expect(requestedBatches[1][0]).toBe('sync_bulk_0200')
    expect(requestedBatches[2][0]).toBe('sync_bulk_0400')
    expect(requestedBatches.every(batch => batch.length <= 200)).toBe(true)
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(false)
    expect(syncJobs.refreshObservedSource).toHaveBeenCalledTimes(1)
  })

  it('批量观察任务在 deadline 到期前保持 pending 判定，到期后修剪', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-05-01T00:00:00.000Z'))
    const syncJobs = setupSyncJobs()
    fetchSyncJobsMock.mockResolvedValue([])

    syncJobs.trackSubmittedJobs(perfTaskIds(3).map(taskId => syncJob({
      task_id: taskId,
      status: 'queued',
    })))

    expect(syncJobs.hasPendingSyncJobObserve()).toBe(true)
    await vi.advanceTimersByTimeAsync(179_999)
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(true)
    await vi.advanceTimersByTimeAsync(2)
    expect(syncJobs.hasPendingSyncJobObserve()).toBe(false)
  })

})

function setupSyncJobs() {
  const refreshObservedSource = vi.fn(async () => {})
  const syncRecords = ref<SyncRecord[]>([])
  return {
    ...useDataCenterSyncJobs({
      syncRecords,
      refreshObservedSource,
    }),
    refreshObservedSource,
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

async function waitFor(predicate: () => boolean) {
  for (let index = 0; index < 20; index += 1) {
    await Promise.resolve()
    if (predicate()) return
  }
  throw new Error('condition not reached')
}

function perfTaskIds(count: number) {
  return Array.from({ length: count }, (_, index) => `sync_bulk_${index.toString().padStart(4, '0')}`)
}
