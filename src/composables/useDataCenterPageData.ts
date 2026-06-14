import { ref, type Ref } from 'vue'
import * as api from '@/api/market'
import type { SyncJob, SyncRecord, SyncRuntimeConfig, WatchedSymbol } from '@/types'
import type { GuardianConfig, GuardianPlan, InventoryRow, InventorySummary } from '@/types/dataCenter'
import { syncRecordScopeKey } from '@/utils/dataCenter'
import { describeError, logger } from '@/utils/logger'

type InventoryPayload = {
  summary: InventorySummary
  rows: InventoryRow[]
}

type DataCenterPageDataOptions = {
  syncRecords: Ref<SyncRecord[]>
  syncRuntimeConfig: Ref<SyncRuntimeConfig | null>
  applyInventoryPayload: (inventory: InventoryPayload) => SyncRecord[]
  replaceSyncRecordScopes: (records: SyncRecord[], scopeKeys: Set<string>) => SyncRecord[]
  applyFetchedSyncJobs: (jobs: SyncJob[], records?: SyncRecord[]) => SyncJob[]
  applyGuardianConfig: (config: GuardianConfig, fallbackPlans?: GuardianPlan[]) => GuardianPlan[]
  error: Ref<string>
}

export function useDataCenterPageData(options: DataCenterPageDataOptions) {
  const watchedSymbols = ref<WatchedSymbol[]>([])
  const loading = ref(false)
  let loadSequence = 0
  let progressRecordLoadSequence = 0
  let progressJobLoadSequence = 0

  async function loadPageData() {
    const sequence = ++loadSequence
    loading.value = true
    const watchedPromise = api.fetchWatchedSymbols()
    const detailResultsPromise = Promise.allSettled([
      api.fetchInventory(),
      api.fetchSyncJobs({ limit: 200, watched_only: true }),
      api.fetchGuardianConfig(),
      api.fetchSyncRuntimeConfig(),
    ])

    const failures: string[] = []
    try {
      const watched = await watchedPromise
      if (sequence !== loadSequence) return
      watchedSymbols.value = watched
    } catch (err) {
      if (sequence !== loadSequence) return
      failures.push('关注币种')
      logger.error('watched symbols refresh failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    }

    const [
      inventoryResult,
      jobsResult,
      guardianConfigResult,
      runtimeConfigResult,
    ] = await detailResultsPromise
    if (sequence !== loadSequence) return

    let records = options.syncRecords.value
    if (inventoryResult.status === 'fulfilled') {
      records = options.applyInventoryPayload(inventoryResult.value)
    } else {
      failures.push('库存')
      logger.error('inventory state refresh failed', {
        scope: 'data-center',
        error: describeError(inventoryResult.reason),
        raw: inventoryResult.reason,
      })
    }

    if (jobsResult.status === 'fulfilled') {
      options.applyFetchedSyncJobs(jobsResult.value, records)
    } else {
      failures.push('同步任务')
      logger.error('sync jobs refresh failed', {
        scope: 'data-center',
        error: describeError(jobsResult.reason),
        raw: jobsResult.reason,
      })
    }

    if (guardianConfigResult.status === 'fulfilled') {
      options.applyGuardianConfig(guardianConfigResult.value, [])
    } else {
      failures.push('Guardian 配置')
      logger.error('guardian config refresh failed', {
        scope: 'data-center',
        error: describeError(guardianConfigResult.reason),
        raw: guardianConfigResult.reason,
      })
    }

    if (runtimeConfigResult.status === 'fulfilled') {
      options.syncRuntimeConfig.value = runtimeConfigResult.value
    } else {
      failures.push('采集参数')
      logger.error('sync runtime config refresh failed', {
        scope: 'data-center',
        error: describeError(runtimeConfigResult.reason),
        raw: runtimeConfigResult.reason,
      })
    }

    options.error.value = failures.length > 0
      ? `数据中心部分状态刷新失败：${failures.join('、')}，详细错误已写入控制台`
      : ''
    loading.value = false
  }

  async function refreshSyncProgressData() {
    const recordSequence = ++progressRecordLoadSequence
    const jobSequence = ++progressJobLoadSequence
    try {
      const [records, jobs] = await Promise.all([
        api.fetchSyncRecords({ watched_only: true }),
        api.fetchSyncJobs({ limit: 200, watched_only: true }),
      ])
      if (recordSequence !== progressRecordLoadSequence) return
      const mergedRecords = options.replaceSyncRecordScopes(
        records,
        watchedSyncRecordScopes(watchedSymbols.value),
      )
      if (jobSequence === progressJobLoadSequence) {
        options.applyFetchedSyncJobs(jobs, mergedRecords)
      }
    } catch (err) {
      warnSyncProgressRefreshFailed(err)
    }
  }

  async function refreshSyncJobProgressData() {
    const sequence = ++progressJobLoadSequence
    try {
      const jobs = await api.fetchSyncJobs({ limit: 200, watched_only: true })
      if (sequence !== progressJobLoadSequence) return
      options.applyFetchedSyncJobs(jobs, options.syncRecords.value)
    } catch (err) {
      warnSyncProgressRefreshFailed(err)
    }
  }

  return {
    watchedSymbols,
    loading,
    loadPageData,
    refreshSyncProgressData,
    refreshSyncJobProgressData,
  }
}

function watchedSyncRecordScopes(items: WatchedSymbol[]) {
  const scopes = new Set<string>()
  for (const item of items) {
    if (item.sync_spot) scopes.add(syncRecordScopeKey(item.spot_inst_id, 'SPOT'))
    if (item.sync_swap) scopes.add(syncRecordScopeKey(item.swap_inst_id, 'SWAP'))
  }
  return scopes
}

function warnSyncProgressRefreshFailed(err: unknown) {
  logger.warn('sync progress refresh failed', {
    scope: 'data-center',
    error: describeError(err),
    raw: err,
  })
}
