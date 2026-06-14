import type { SyncJob } from '@/types'

import { bounded, clampPercent, clampProgress, nonNegative } from './numbers'
import { syncJobSortTime } from './time'
import { classifySyncJobFailure, syncJobFailureDetail } from './failures'

export function newestSyncJob(jobs: SyncJob[]) {
  return jobs.reduce<SyncJob | null>((latest, job) => {
    if (!latest || syncJobSortTime(job) >= syncJobSortTime(latest)) return job
    return latest
  }, null)
}

export function mergeSyncJobsByTaskId(current: SyncJob[], incoming: SyncJob[]) {
  if (incoming.length === 0) return current
  const byId = new Map<string, SyncJob>()
  for (const job of current) {
    if (job.task_id) byId.set(job.task_id, job)
  }
  for (const job of incoming) {
    if (!job.task_id) continue
    const existing = byId.get(job.task_id)
    if (!existing || syncJobSortTime(job) >= syncJobSortTime(existing)) {
      byId.set(job.task_id, job)
    }
  }
  return Array.from(byId.values()).sort(
    (left, right) => syncJobSortTime(right) - syncJobSortTime(left)
  )
}

export function jobProgressPercent(job: SyncJob) {
  if (job.status === 'completed') return 100
  if (job.status === 'queued') return 0
  if (job.status === 'failed' || job.status === 'cancelled') return terminalJobProgress(job)
  return clampProgress(job.progress)
}

export function isDerivePhaseJob(job: SyncJob | null) {
  if (!job || job.status !== 'running') return false
  const message = job.message || ''
  return (
    nonNegative(job.target_derive_count) > 0 &&
    (
      nonNegative(job.derived_count) > 0 ||
      clampProgress(job.progress) >= 88 ||
      message.includes('派生') ||
      message.includes('对齐')
    )
  )
}

export function isSavePhaseJob(job: SyncJob | null) {
  if (!job || job.status !== 'running') return false
  const message = job.message || ''
  const progress = clampProgress(job.progress)
  const fetched = nonNegative(job.fetched_count)
  const saved = nonNegative(job.saved_count)
  const targetSave = nonNegative(job.target_save_count)
  return (
    message.startsWith('落库') ||
    message.includes('基础 K 线同步完成') ||
    (targetSave > 0 && fetched > 0 && saved < targetSave && progress >= 68)
  )
}

export function isFetchPhaseJob(job: SyncJob | null) {
  if (!job || job.status !== 'running') return false
  const message = job.message || ''
  return (
    message.includes('拉取') ||
    message.includes('回补') ||
    nonNegative(job.target_fetch_count) > 0
  )
}

export function formatJobFailure(job: SyncJob | null) {
  const detail = syncJobFailureDetail(job)
  if (!detail) return '同步失败'
  const failureKind = classifySyncJobFailure(detail)
  if (failureKind === 'rate-limit') return 'OKX 限流'
  if (failureKind === 'bar-parameter') return 'OKX 周期参数错误'
  if (failureKind === 'disabled-target') return '未启用该目标'
  return detail.length > 24 ? `${detail.slice(0, 24)}...` : detail
}

function terminalJobProgress(job: SyncJob) {
  const countProgress = countBasedJobProgress(job)
  if (countProgress > 0) return countProgress
  const progress = clampProgress(job.progress)
  return progress
}

function countBasedJobProgress(job: SyncJob) {
  const fetched = nonNegative(job.fetched_count)
  const targetFetch = nonNegative(job.target_fetch_count)
  const saved = nonNegative(job.saved_count)
  const targetSave = nonNegative(job.target_save_count)
  const derived = nonNegative(job.derived_count)
  const targetDerive = nonNegative(job.target_derive_count)
  const total = targetFetch + targetSave + targetDerive

  if (total > 0) {
    const done = bounded(fetched, targetFetch)
      + bounded(saved, targetSave)
      + bounded(derived, targetDerive)
    return clampPercent(Math.round((bounded(done, total) * 100) / total))
  }

  return 0
}
