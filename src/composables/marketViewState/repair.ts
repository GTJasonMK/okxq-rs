import { ref, type ComputedRef } from 'vue'
import type { Router } from 'vue-router'
import * as api from '@/api/market'
import type { SyncJob, WatchedSymbol } from '@/types'
import type { useMarketStore } from '@/stores/marketStore'
import type { RepairProgress } from '@/types/marketView'
import {
  mergeSyncJobsByTaskId,
  nextObservedTaskBatch,
  rotateObservedTaskBatch,
  summarizeSyncProgress,
} from '@/utils/syncProgress'
import { describeError } from '@/utils/logger'

type MarketStore = ReturnType<typeof useMarketStore>

export function useMarketRepairState(options: {
  activeBaseSymbol: ComputedRef<string>
  activeWatchedSymbol: ComputedRef<WatchedSymbol | null>
  loadCandles: () => Promise<void>
  loadMarketSnapshot: () => Promise<void>
  router: Router
  store: MarketStore
}) {
  const {
    activeBaseSymbol,
    activeWatchedSymbol,
    loadCandles,
    loadMarketSnapshot,
    router,
    store,
  } = options

  const repairing = ref(false)
  const repairProgress = ref<RepairProgress>({
    visible: false,
    ...summarizeSyncProgress([]),
  })

  async function repairActive() {
    const watched = activeWatchedSymbol.value
    if (!watched) {
      openDataCenter()
      return
    }
    repairing.value = true
    resetRepairProgress()
    try {
      const result = await api.repairWatchedSymbol(watched.symbol, {
        sync_spot: watched.sync_spot,
        sync_swap: watched.sync_swap,
      })
      applyRepairProgress(result.sync_jobs ?? [])
      await waitForSubmittedJobs(result.sync_jobs ?? [])
      await loadCandles()
      await loadMarketSnapshot()
    } catch (e) {
      store.error = describeError(e)
    } finally {
      repairing.value = false
    }
  }

  function openDataCenter() {
    void router.push({
      path: '/data-center',
      query: activeBaseSymbol.value ? { symbol: activeBaseSymbol.value } : {},
    })
  }

  async function waitForSubmittedJobs(jobs: SyncJob[]) {
    const taskIds = jobs.map(job => job.task_id).filter(Boolean)
    if (taskIds.length === 0) return
    let displayedJobs = jobs
    applyRepairProgress(displayedJobs)
    const pending = new Set(taskIds)
    const deadline = Date.now() + 180_000
    while (pending.size > 0 && Date.now() < deadline) {
      const batchTaskIds = nextObservedTaskBatch(pending)
      const latest = await api.fetchSyncJobs({
        task_ids: batchTaskIds,
        limit: batchTaskIds.length,
      }, { dedupe: false })
      displayedJobs = mergeSyncJobsByTaskId(displayedJobs, latest)
      applyRepairProgress(displayedJobs)
      for (const job of latest) {
        if (!['queued', 'running'].includes(job.status)) pending.delete(job.task_id)
      }
      rotateObservedTaskBatch(pending, batchTaskIds)
      if (pending.size > 0) await delay(1200)
    }
  }

  function resetRepairProgress() {
    repairProgress.value = {
      visible: true,
      ...summarizeSyncProgress([]),
      statusLabel: '同步准备中',
      phaseLabel: '等待执行',
      primaryText: '等待调度',
    }
  }

  function applyRepairProgress(jobs: SyncJob[]) {
    if (jobs.length === 0) return
    const summary = summarizeSyncProgress(jobs)
    repairProgress.value = {
      visible: true,
      ...summary,
    }
  }

  function delay(ms: number) {
    return new Promise(resolve => window.setTimeout(resolve, ms))
  }

  return {
    repairing,
    repairProgress,
    repairActive,
    openDataCenter,
  }
}
