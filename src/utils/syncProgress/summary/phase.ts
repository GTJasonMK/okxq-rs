import type { SyncJob } from '@/types'
import {
  isDerivePhaseJob,
  isFetchPhaseJob,
  isSavePhaseJob,
} from '@/utils/syncProgress/jobs'
import type { SyncPhase, SyncProgressSummary } from '@/utils/syncProgress/types'

export function inferPhase(summary: SyncProgressSummary, latest: SyncJob | null, jobs: SyncJob[]): SyncPhase {
  if (summary.active === 0) {
    if (summary.failed > 0) return 'failed'
    if (summary.cancelled > 0) return 'cancelled'
    return 'completed'
  }
  if (latest?.status === 'queued' && summary.running === 0) return 'queued'

  const runningJobs = jobs.filter(job => job.status === 'running')
  if (runningJobs.some(isDerivePhaseJob) || isDerivePhaseJob(latest)) return 'derive'
  if (runningJobs.some(isSavePhaseJob) || isSavePhaseJob(latest)) return 'save'
  if (runningJobs.some(isFetchPhaseJob) || isFetchPhaseJob(latest)) return 'fetch'
  if (summary.targetDerive > 0 && summary.derived < summary.targetDerive) return 'derive'
  if (summary.targetSave > 0 && summary.saved < summary.targetSave) return 'save'
  if (summary.targetFetch > 0 && summary.fetched < summary.targetFetch) return 'fetch'
  return 'running'
}
