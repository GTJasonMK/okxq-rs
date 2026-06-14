import { computed, ref, type Ref } from 'vue'
import * as api from '@/api/market'
import type { SyncJob, SyncRecord } from '@/types'
import { describeError, logger } from '@/utils/logger'
import {
  nextObservedTaskBatch,
  rotateObservedTaskBatch,
  syncJobSortTime,
} from '@/utils/syncProgress'
import {
  TERMINAL_SYNC_JOB_VISIBLE_MS,
  activeSyncJobs,
  buildSyncRecordTargetIndex,
  isObservedSyncJob,
  mergeSyncJobs,
  syncJobSupersededByRecordIndex,
  visibleSyncJobs,
} from '@/utils/dataCenter'

const SYNC_JOB_OBSERVE_MS = 180_000
const SYNC_JOB_POLL_MS = 1200
const FAILED_JOB_LOG_GRACE_MS = 10_000

type DataCenterSyncJobsOptions = {
  syncRecords: Ref<SyncRecord[]>
  refreshObservedSource: () => Promise<void> | void
}

export function useDataCenterSyncJobs(options: DataCenterSyncJobsOptions) {
  const syncJobs = ref<SyncJob[]>([])
  const activeJobs = computed(() => activeSyncJobs(syncJobs.value))
  const loggedFailedJobKeys = new Set<string>()
  const observedSyncJobDeadlines = new Map<string, number>()
  const pendingObservedTaskIds = new Set<string>()
  const failedJobLogStartedAt = Date.now()
  let nextObservedPruneAt = Number.POSITIVE_INFINITY

  function applyFetchedSyncJobs(jobs: SyncJob[], records = options.syncRecords.value) {
    markTerminalObservedJobs(jobs)
    const nextJobs = hasVisibleObservedSyncJobs()
      ? mergeSyncJobs(syncJobs.value, jobs, records, observedSyncJobDeadlines)
      : visibleSyncJobs(jobs, records, observedSyncJobDeadlines)
    syncJobs.value = nextJobs
    logFailedSyncJobs(nextJobs, records)
    return nextJobs
  }

  function trackSubmittedJobs(jobs: SyncJob[]) {
    const taskIds = jobs.map(job => job.task_id).filter(Boolean)
    if (taskIds.length === 0) return
    const deadline = Date.now() + SYNC_JOB_OBSERVE_MS
    taskIds.forEach((taskId) => {
      observedSyncJobDeadlines.set(taskId, deadline)
      pendingObservedTaskIds.add(taskId)
    })
    rememberObservedDeadline(deadline)
    prunePendingSyncJobObserve()
    syncJobs.value = mergeSyncJobs(
      syncJobs.value,
      jobs,
      options.syncRecords.value,
      observedSyncJobDeadlines,
    )
    void observeSubmittedJobs(taskIds, deadline).catch((err) => {
      releasePendingObservedTasks(taskIds, deadline)
      logger.warn('sync job progress observe failed', {
        scope: 'data-center',
        task_ids: taskIds,
        error: describeError(err),
        raw: err,
      })
    })
  }

  function hasPendingSyncJobObserve() {
    prunePendingSyncJobObserve()
    return pendingObservedTaskIds.size > 0
  }

  function shouldRefreshSyncJobSource() {
    prunePendingSyncJobObserve()
    return activeJobs.value.some(job => !isPendingObservedTaskId(job.task_id))
  }

  async function observeSubmittedJobs(taskIds: string[], deadline: number) {
    const pending = new Set(taskIds)
    let shouldRefreshSource = false
    while (pending.size > 0 && Date.now() < deadline) {
      const batchTaskIds = nextObservedTaskBatch(pending)
      const latest = await api.fetchSyncJobs(
        { task_ids: batchTaskIds, limit: batchTaskIds.length },
        { dedupe: false },
      )
      syncJobs.value = mergeSyncJobs(
        syncJobs.value,
        latest,
        options.syncRecords.value,
        observedSyncJobDeadlines,
      )
      logFailedSyncJobs(latest, options.syncRecords.value)
      for (const job of latest) {
        if (isActiveSyncJob(job)) continue
        if (pending.delete(job.task_id)) {
          pendingObservedTaskIds.delete(job.task_id)
          shouldRefreshSource = true
        }
      }
      rotateObservedTaskBatch(pending, batchTaskIds)
      if (pending.size > 0) await delay(SYNC_JOB_POLL_MS)
    }
    if (pending.size > 0) {
      releasePendingObservedTasks(Array.from(pending), deadline)
      shouldRefreshSource = true
    }
    if (shouldRefreshSource) await options.refreshObservedSource()
  }

  function logFailedSyncJobs(jobs: SyncJob[], records: SyncRecord[]) {
    let recordIndex: ReturnType<typeof buildSyncRecordTargetIndex> | undefined
    for (const job of jobs) {
      if (job.status !== 'failed' || !job.error) continue
      if (!shouldLogFailedJob(job)) continue
      recordIndex ??= buildSyncRecordTargetIndex(records)
      if (syncJobSupersededByRecordIndex(job, recordIndex)) continue
      const logKey = `${job.task_id}:${job.updated_at || job.finished_at || ''}:${job.error}`
      if (loggedFailedJobKeys.has(logKey)) continue
      loggedFailedJobKeys.add(logKey)
      logger.error('sync job failed', {
        scope: 'data-center',
        task_id: job.task_id,
        inst_id: job.inst_id,
        inst_type: job.inst_type,
        timeframe: job.timeframe,
        source_timeframe: job.source_timeframe,
        target_timeframes: job.target_timeframes,
        mode: job.mode,
        status: job.status,
        error: job.error,
        message: job.message,
        fetched_count: job.fetched_count,
        saved_count: job.saved_count,
        derived_count: job.derived_count,
        candle_count: job.candle_count,
        updated_at: job.updated_at,
        finished_at: job.finished_at,
      })
    }
  }

  function shouldLogFailedJob(job: SyncJob) {
    const now = Date.now()
    if (isObservedSyncJob(job.task_id, observedSyncJobDeadlines, now)) return true
    const updatedAt = syncJobSortTime(job)
    return (
      updatedAt >= failedJobLogStartedAt - FAILED_JOB_LOG_GRACE_MS &&
      now - updatedAt <= TERMINAL_SYNC_JOB_VISIBLE_MS
    )
  }

  function hasVisibleObservedSyncJobs() {
    prunePendingSyncJobObserve()
    return observedSyncJobDeadlines.size > 0
  }

  function prunePendingSyncJobObserve(now = Date.now()) {
    if (now <= nextObservedPruneAt) return
    nextObservedPruneAt = Number.POSITIVE_INFINITY
    for (const [taskId, deadline] of observedSyncJobDeadlines) {
      if (deadline < now) {
        observedSyncJobDeadlines.delete(taskId)
        pendingObservedTaskIds.delete(taskId)
      } else {
        rememberObservedDeadline(deadline)
      }
    }
    for (const taskId of pendingObservedTaskIds) {
      if (!isObservedSyncJob(taskId, observedSyncJobDeadlines, now)) {
        pendingObservedTaskIds.delete(taskId)
      }
    }
  }

  function rememberObservedDeadline(deadline: number) {
    if (deadline < nextObservedPruneAt) nextObservedPruneAt = deadline
  }

  function markTerminalObservedJobs(jobs: SyncJob[]) {
    for (const job of jobs) {
      if (isActiveSyncJob(job)) continue
      pendingObservedTaskIds.delete(job.task_id)
    }
  }

  function releasePendingObservedTasks(taskIds: string[], deadline: number) {
    for (const taskId of taskIds) {
      if ((observedSyncJobDeadlines.get(taskId) ?? 0) <= deadline) {
        pendingObservedTaskIds.delete(taskId)
      }
    }
  }

  function isPendingObservedTaskId(taskId: string) {
    return Boolean(taskId && pendingObservedTaskIds.has(taskId))
  }

  return {
    syncJobs,
    activeJobs,
    applyFetchedSyncJobs,
    trackSubmittedJobs,
    hasPendingSyncJobObserve,
    shouldRefreshSyncJobSource,
  }
}

function isActiveSyncJob(job: SyncJob) {
  return job.status === 'queued' || job.status === 'running'
}

function delay(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}
