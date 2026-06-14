import type { SyncJob } from '@/types'

export function activeSyncJobs(jobs: SyncJob[]) {
  return jobs.filter(job => ['queued', 'running'].includes(job.status))
}
