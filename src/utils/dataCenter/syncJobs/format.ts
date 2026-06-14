import type { SyncJob } from '@/types'
import { jobProgressPercent } from '@/utils/syncProgress'
import { classifySyncJobFailure, syncJobFailureDetail } from '@/utils/syncProgress/failures'

export function formatJobStatus(job: SyncJob) {
  if (job.status === 'running') return `${jobProgressPercent(job)}%`
  if (job.status === 'queued') return '排队'
  if (job.status === 'completed') return '完成'
  if (job.status === 'failed') return '失败'
  if (job.status === 'cancelled') return '已取消'
  return job.status
}

export function formatJobFailure(job: SyncJob) {
  const detail = syncJobFailureDetail(job)
  if (!detail) return '补齐失败'
  const failureKind = classifySyncJobFailure(detail)
  if (failureKind === 'rate-limit') return '补齐失败：OKX限流'
  if (failureKind === 'disabled-target') return '补齐失败：未启用'
  return `补齐失败：${detail.slice(0, 18)}`
}
