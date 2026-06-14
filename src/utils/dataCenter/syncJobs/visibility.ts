import type { SyncJob, SyncRecord } from '@/types'
import { mergeSyncJobsByTaskId, syncJobSortTime } from '@/utils/syncProgress'
import {
  buildSyncRecordTargetIndex,
  syncJobSupersededByRecordIndex,
  type SyncRecordTargetIndex,
} from '@/utils/dataCenter/syncJobs/targets'

export const TERMINAL_SYNC_JOB_VISIBLE_MS = 30 * 60_000
type SyncRecordTargetIndexReader = () => SyncRecordTargetIndex

export function mergeSyncJobs(
  current: SyncJob[],
  incoming: SyncJob[],
  records: SyncRecord[],
  observedDeadlines: Map<string, number>,
  now = Date.now(),
) {
  if (incoming.length === 0) return current
  return visibleSyncJobs(mergeSyncJobsByTaskId(current, incoming), records, observedDeadlines, now)
}

export function visibleSyncJobs(
  jobs: SyncJob[],
  records: SyncRecord[],
  observedDeadlines: Map<string, number>,
  now = Date.now(),
) {
  const getRecordIndex = lazySyncRecordTargetIndex(records)
  return jobs
    .filter(job => shouldKeepVisibleSyncJobWithRecordIndex(job, getRecordIndex, observedDeadlines, now))
    .sort((left, right) => syncJobSortTime(right) - syncJobSortTime(left))
    .slice(0, 200)
}

function shouldKeepVisibleSyncJobWithRecordIndex(
  job: SyncJob,
  getRecordIndex: SyncRecordTargetIndexReader,
  observedDeadlines: Map<string, number>,
  now: number,
) {
  if (['queued', 'running'].includes(job.status)) return true
  if (job.status === 'failed' && syncJobSupersededByRecordIndex(job, getRecordIndex())) return false
  if (isObservedSyncJob(job.task_id, observedDeadlines, now)) return true
  const updatedAt = syncJobSortTime(job)
  return updatedAt > 0 && now - updatedAt <= TERMINAL_SYNC_JOB_VISIBLE_MS
}

function lazySyncRecordTargetIndex(records: SyncRecord[]): SyncRecordTargetIndexReader {
  let recordIndex: SyncRecordTargetIndex | undefined
  return () => {
    recordIndex ??= buildSyncRecordTargetIndex(records)
    return recordIndex
  }
}

export function isObservedSyncJob(
  taskId: string,
  observedDeadlines: Map<string, number>,
  now = Date.now(),
) {
  return Boolean(taskId && (observedDeadlines.get(taskId) ?? 0) >= now)
}
