import { ref, type ComputedRef, type Ref } from 'vue'
import * as api from '@/api/market'
import type {
  SyncJob,
  SyncRuntimeConfig,
  SyncRuntimeSettings,
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import type { WatchedRow } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'
import { normalizeFullSyncPlans } from '@/utils/syncPlans'
import {
  normalizeInputSymbol,
  repairWatchedSymbolMessage,
  sameSyncRuntimeSettings,
  syncTaskSubmissionSummary,
  watchRuleSavedAction,
} from '@/utils/dataCenter'

type WatchRuleRow = Omit<WatchedRow, 'jobs' | 'jobSummary'>

type WatchRuleSubmitState = {
  newSymbol: Ref<string>
  pendingRuleSymbol: Ref<string>
  syncSpot: Ref<boolean>
  syncSwap: Ref<boolean>
  archiveAll: Ref<boolean>
  autoSync: Ref<boolean>
  syncDays: Ref<number>
  syncPlans: Ref<WatchedSymbolSyncPlan[]>
  canSubmit: ComputedRef<boolean>
  resetAfterSaved: () => void
}

type DataCenterWatchlistActionsOptions = {
  message: Ref<string>
  error: Ref<string>
  adding: Ref<boolean>
  syncRuntimeConfig: Ref<SyncRuntimeConfig | null>
  watchedRows: ComputedRef<WatchRuleRow[]>
  rule: WatchRuleSubmitState
  clearFeedback: () => void
  loadPageData: () => Promise<void>
  trackSubmittedJobs: (jobs: SyncJob[]) => void
}

export function useDataCenterWatchlistActions(options: DataCenterWatchlistActionsOptions) {
  const savingSyncRuntime = ref(false)
  const repairingSymbol = ref('')
  const deletingSymbol = ref('')

  async function saveSyncRuntimeConfig(settings: SyncRuntimeSettings) {
    options.clearFeedback()
    savingSyncRuntime.value = true
    try {
      options.syncRuntimeConfig.value = await api.updateSyncRuntimeConfig(settings)
      options.message.value = '数据采集性能参数已保存，后续新建同步任务生效'
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('sync runtime config save failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      savingSyncRuntime.value = false
    }
  }

  function submitRuleDialog(settings?: SyncRuntimeSettings) {
    void addSymbol(settings)
  }

  async function addSymbol(runtimeSettings?: SyncRuntimeSettings) {
    const rule = options.rule
    const symbol = normalizeInputSymbol(rule.pendingRuleSymbol.value || rule.newSymbol.value)
    if (!symbol || !rule.canSubmit.value) return
    options.clearFeedback()
    options.adding.value = true
    const wasInventoryOnly = options.watchedRows.value.some(row => row.symbol === symbol && row.inventory_only)
    try {
      await saveSyncRuntimeConfigForSubmit(runtimeSettings)
      const syncPlans = normalizeFullSyncPlans(rule.syncPlans.value)
      const result = await api.addWatchedSymbol(symbol, {
        sync_spot: rule.syncSpot.value,
        sync_swap: rule.syncSwap.value,
        archive_all_history: rule.archiveAll.value,
        sync_days: rule.syncDays.value,
        sync_plans: syncPlans,
        auto_sync: rule.autoSync.value,
      })
      const action = watchRuleSavedAction(Boolean(result.existed), wasInventoryOnly)
      options.message.value = rule.autoSync.value
        ? `${symbol} ${action}，${syncTaskSubmissionSummary(result)}`
        : `${symbol} ${action}，未立即提交补齐任务`
      rule.resetAfterSaved()
      options.trackSubmittedJobs(result.sync_jobs ?? [])
      await options.loadPageData()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('watched symbol save failed', {
        scope: 'data-center',
        symbol,
        error: describeError(err),
        raw: err,
      })
    } finally {
      options.adding.value = false
    }
  }

  async function saveSyncRuntimeConfigForSubmit(settings?: SyncRuntimeSettings) {
    if (!settings || !options.syncRuntimeConfig.value) return
    if (sameSyncRuntimeSettings(settings, options.syncRuntimeConfig.value.settings)) return
    savingSyncRuntime.value = true
    try {
      options.syncRuntimeConfig.value = await api.updateSyncRuntimeConfig(settings)
    } finally {
      savingSyncRuntime.value = false
    }
  }

  async function repairSymbol(row: WatchedSymbol) {
    options.clearFeedback()
    repairingSymbol.value = row.symbol
    try {
      const result = await api.repairWatchedSymbol(row.symbol, {
        sync_spot: row.sync_spot,
        sync_swap: row.sync_swap,
      })
      options.message.value = repairWatchedSymbolMessage(row.symbol, result)
      options.trackSubmittedJobs(result.sync_jobs ?? [])
      await options.loadPageData()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('watched symbol repair failed', {
        scope: 'data-center',
        symbol: row.symbol,
        error: describeError(err),
        raw: err,
      })
    } finally {
      repairingSymbol.value = ''
    }
  }

  async function cancelRowActiveJobs(row: WatchedRow) {
    const jobs = row.jobs.filter(job => ['queued', 'running'].includes(job.status))
    if (jobs.length === 0) return
    options.clearFeedback()
    try {
      await Promise.all(jobs.map(job => api.cancelSyncJob(job.task_id)))
      options.message.value = `${row.symbol} 已取消 ${jobs.length} 个运行中的同步任务`
      await options.loadPageData()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('active sync jobs cancel failed', {
        scope: 'data-center',
        symbol: row.symbol,
        error: describeError(err),
        raw: err,
      })
    }
  }

  async function deleteSymbol(symbol: string) {
    options.clearFeedback()
    deletingSymbol.value = symbol
    try {
      await api.deleteWatchedSymbol(symbol)
      options.message.value = `${symbol} 已移出关注清单，相关本地数据和运行中任务已清理`
      await options.loadPageData()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('watched symbol delete failed', {
        scope: 'data-center',
        symbol,
        error: describeError(err),
        raw: err,
      })
    } finally {
      deletingSymbol.value = ''
    }
  }

  return {
    adding: options.adding,
    savingSyncRuntime,
    repairingSymbol,
    deletingSymbol,
    syncRuntimeConfig: options.syncRuntimeConfig,
    saveSyncRuntimeConfig,
    submitRuleDialog,
    addSymbol,
    repairSymbol,
    cancelRowActiveJobs,
    deleteSymbol,
  }
}
