import type { SyncJob } from '@/types'
import { jobProgressPercent, newestSyncJob } from '@/utils/syncProgress/jobs'
import { clampPercent, nonNegative } from '@/utils/syncProgress/numbers'
import type { SyncProgressSummary } from '@/utils/syncProgress/types'
import { emptySummary } from '@/utils/syncProgress/summary/empty'
import {
  phaseLabel,
  primaryText,
  secondaryText,
  statusLabel,
  taskText,
} from '@/utils/syncProgress/summary/labels'
import { inferPhase } from '@/utils/syncProgress/summary/phase'
import { buildSegments } from '@/utils/syncProgress/summary/segments'

export function summarizeSyncProgress(jobs: SyncJob[]): SyncProgressSummary {
  const summary = emptySummary()
  const activeJobs = jobs.filter(job => ['queued', 'running'].includes(job.status))
  const displayJobs = activeJobs.length > 0 ? activeJobs : jobs
  let progressTotal = 0
  let progressCount = 0

  for (const job of displayJobs) {
    summary.total += 1
    if (job.status === 'queued') summary.queued += 1
    if (job.status === 'running') summary.running += 1
    if (job.status === 'completed') summary.completed += 1
    if (job.status === 'failed') summary.failed += 1
    if (job.status === 'cancelled') summary.cancelled += 1

    progressTotal += jobProgressPercent(job)
    progressCount += 1

    summary.fetched += nonNegative(job.fetched_count)
    summary.targetFetch += nonNegative(job.target_fetch_count)
    summary.saved += nonNegative(job.saved_count)
    summary.targetSave += nonNegative(job.target_save_count)
    summary.derived += nonNegative(job.derived_count)
    summary.targetDerive += nonNegative(job.target_derive_count)
    summary.batches += nonNegative(job.batches)
    summary.targetBatches += nonNegative(job.target_batches)
    summary.apiCalls += nonNegative(job.api_calls)
  }

  summary.active = summary.queued + summary.running
  summary.progress = progressCount > 0 ? clampPercent(Math.round(progressTotal / progressCount)) : 0
  summary.statusLabel = statusLabel(summary)
  summary.taskText = taskText(summary)

  const latest = newestSyncJob(displayJobs)
  const phase = inferPhase(summary, latest, displayJobs)
  summary.phaseLabel = phaseLabel(phase)
  summary.primaryText = primaryText(summary, phase, latest)
  summary.secondaryText = secondaryText(summary)
  summary.segments = buildSegments(summary, phase)
  return summary
}
