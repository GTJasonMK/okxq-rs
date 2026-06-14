import type { InstType, SyncJob, SyncRecord } from '@/types'
import { summarizeSyncProgress, syncJobSortTime } from '@/utils/syncProgress'
import {
  syncJobSupersededByRecords,
  syncJobTargetTimeframes,
} from '@/utils/dataCenter/syncJobs/targets'

export function summarizeJobs(jobs: SyncJob[]) {
  return summarizeSyncProgress(latestJobsByScope(jobs))
}

function latestJobsByScope(jobs: SyncJob[]) {
  const latest = new Map<string, SyncJob>()
  for (const job of jobs) {
    const targets = syncJobTargetTimeframes(job).join('/')
    const key = `${job.inst_id}:${job.inst_type}:${targets}`
    const existing = latest.get(key)
    if (!existing || shouldReplaceScopedJob(job, existing)) {
      latest.set(key, job)
    }
  }
  return Array.from(latest.values())
}

export function jobRelevantToRow(
  job: SyncJob,
  enabledScopes: Array<{ instId: string; instType: InstType }>,
  effectiveTimeframes: Set<string>,
  records: SyncRecord[],
) {
  if (!enabledScopes.some(scope => job.inst_id === scope.instId && job.inst_type === scope.instType)) {
    return false
  }
  if (['queued', 'running'].includes(job.status)) return true

  const targets = syncJobTargetTimeframes(job)
  if (effectiveTimeframes.size > 0 && !targets.some(timeframe => effectiveTimeframes.has(timeframe))) {
    return false
  }
  if (job.status === 'failed' && syncJobSupersededByRecords(job, records, targets)) {
    return false
  }
  return true
}

export function newestJob(jobs: SyncJob[]) {
  return jobs.reduce<SyncJob | undefined>((latest, job) => {
    if (!latest || jobSortTime(job) > jobSortTime(latest)) return job
    return latest
  }, undefined)
}

function jobSortTime(job: SyncJob) {
  return syncJobSortTime(job)
}

function shouldReplaceScopedJob(candidate: SyncJob, existing: SyncJob) {
  const candidateActive = isActiveJob(candidate)
  const existingActive = isActiveJob(existing)
  if (candidateActive !== existingActive) return candidateActive
  return jobSortTime(candidate) > jobSortTime(existing)
}

function isActiveJob(job: SyncJob) {
  return job.status === 'queued' || job.status === 'running'
}
