import type { SyncJob } from '@/types'

export type SyncJobFailureKind =
  | 'bar-parameter'
  | 'disabled-target'
  | 'generic'
  | 'rate-limit'

export function syncJobFailureDetail(job: Pick<SyncJob, 'error' | 'message'> | null | undefined) {
  return String(job?.error || job?.message || '').trim()
}

export function classifySyncJobFailure(detail: string): SyncJobFailureKind {
  if (detail.includes('429') || detail.includes('Too Many Requests') || detail.includes('50011')) {
    return 'rate-limit'
  }
  if (detail.includes('Parameter bar error') || detail.includes('51000')) {
    return 'bar-parameter'
  }
  if (detail.includes('未在关注清单中启用')) {
    return 'disabled-target'
  }
  return 'generic'
}
