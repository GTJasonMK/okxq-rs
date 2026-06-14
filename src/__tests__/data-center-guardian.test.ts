import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterGuardian } from '@/composables/useDataCenterGuardian'
import type { SyncJob } from '@/types'
import type { GuardianConfig, GuardianPlan, GuardianStatus } from '@/types/dataCenter'

vi.mock('@/api/market', () => ({
  fetchGuardianStatus: vi.fn(),
  fetchGuardianConfig: vi.fn(),
}))

const fetchGuardianStatusMock = vi.mocked(marketApi.fetchGuardianStatus)
const fetchGuardianConfigMock = vi.mocked(marketApi.fetchGuardianConfig)

describe('useDataCenterGuardian', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('加载 Guardian 状态和配置后生成展示派生字段', async () => {
    const state = feedback()
    const guardian = useDataCenterGuardian(state)
    fetchGuardianStatusMock.mockResolvedValue(guardianStatus({
      current_inst_id: 'BTC-USDT-SWAP',
      current_timeframe: '1H',
      current_mode: 'repair',
      last_errors: [' first ', { message: ' second ' }, { message: 12 }],
      backfill_queue_preview: [syncJob({ task_id: 'queued-job' })],
    }))
    fetchGuardianConfigMock.mockResolvedValue(guardianConfig({
      plans: [
        { timeframe: '1H', enabled: true, bootstrap_days: 90, archive_mode: 'rolling' },
      ],
    }))

    await guardian.loadGuardianData()

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(fetchGuardianStatusMock).toHaveBeenCalledTimes(1)
    expect(fetchGuardianConfigMock).toHaveBeenCalledTimes(1)
    expect(guardian.guardianPlans.value.map(plan => plan.timeframe)).toEqual(['1H'])
    expect(guardian.guardianCurrentTarget.value).toBe('BTC-USDT-SWAP · 1H · repair')
    expect(guardian.guardianErrors.value).toEqual(['first', 'second'])
    expect(guardian.guardianQueuePreview.value.map(job => job.task_id)).toEqual(['queued-job'])
    expect(guardian.guardianStatusLoading.value).toBe(false)
    expect(state.error.value).toBe('')
  })

  it('连续刷新时只保留最后一次状态结果', async () => {
    const guardian = useDataCenterGuardian(feedback())
    const first = deferred<GuardianStatus>()
    const second = deferred<GuardianStatus>()
    fetchGuardianStatusMock
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)

    const firstRun = guardian.refreshGuardianStatus()
    const secondRun = guardian.refreshGuardianStatus()
    second.resolve(guardianStatus({ active: true, current_inst_id: 'ETH-USDT-SWAP' }))
    await secondRun
    first.resolve(guardianStatus({ active: false, current_inst_id: 'BTC-USDT-SWAP' }))
    await firstRun

    expect(guardian.guardianStatus.value).toMatchObject({
      active: true,
      current_inst_id: 'ETH-USDT-SWAP',
    })
  })
})

function feedback() {
  return {
    error: ref(''),
    clearFeedback: vi.fn(),
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
    last_errors: [],
    last_sync_results: [],
    ...overrides,
  }
}

function guardianConfig(overrides: Partial<{ plans: GuardianPlan[] }> = {}): GuardianConfig {
  const plans = overrides.plans ?? []
  return {
    settings: {
      enabled: true,
      scan_interval_seconds: 300,
      max_full_backfill_jobs_per_cycle: 1,
      plans,
    },
    defaults: {
      enabled: true,
      scan_interval_seconds: 300,
      max_full_backfill_jobs_per_cycle: 1,
      plans: [],
    },
  }
}

function syncJob(overrides: Partial<SyncJob> = {}): SyncJob {
  return {
    task_id: 'sync-job',
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    mode: 'repair',
    status: 'queued',
    progress: 0,
    created_at: '2026-01-01T00:00:00.000Z',
    ...overrides,
  }
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  const promise = new Promise<T>((innerResolve) => {
    resolve = innerResolve
  })
  return { promise, resolve }
}
