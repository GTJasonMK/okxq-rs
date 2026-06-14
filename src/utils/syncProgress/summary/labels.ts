import type { SyncJob } from '@/types'
import { formatInteger, formatWorkProgress } from '@/utils/syncProgress/format'
import { formatJobFailure } from '@/utils/syncProgress/jobs'
import type { SyncPhase, SyncProgressSummary } from '@/utils/syncProgress/types'

export function statusLabel(summary: SyncProgressSummary) {
  if (summary.total === 0) return '同步任务'
  if (summary.active > 0) return '同步中'
  if (summary.failed > 0) return '同步失败'
  if (summary.cancelled > 0) return '已取消'
  return '同步完成'
}

export function taskText(summary: SyncProgressSummary) {
  if (summary.total === 0) return '任务 0 / 0'
  const parts = [`任务 ${formatInteger(summary.completed)} / ${formatInteger(summary.total)}`]
  if (summary.running) parts.push(`运行 ${formatInteger(summary.running)}`)
  if (summary.queued) parts.push(`排队 ${formatInteger(summary.queued)}`)
  if (summary.failed) parts.push(`失败 ${formatInteger(summary.failed)}`)
  if (summary.cancelled) parts.push(`取消 ${formatInteger(summary.cancelled)}`)
  return parts.join(' · ')
}

export function phaseLabel(phase: SyncPhase) {
  switch (phase) {
    case 'queued': return '等待执行'
    case 'fetch': return '正在拉取'
    case 'save': return '正在落库'
    case 'derive': return '正在对齐'
    case 'completed': return '已完成'
    case 'failed': return '失败'
    case 'cancelled': return '已取消'
    default: return '收尾中'
  }
}

export function primaryText(summary: SyncProgressSummary, phase: SyncPhase, latest: SyncJob | null) {
  if (phase === 'failed') return formatJobFailure(latest)
  if (phase === 'cancelled') return '任务已取消'
  if (phase === 'queued') return '等待调度'
  if (phase === 'completed') return '任务完成'
  const message = currentJobMessage(latest)
  if (message) return message
  if (phase === 'fetch') return `当前拉取 ${formatWorkProgress(summary.fetched, summary.targetFetch)}`
  if (phase === 'save') return `当前写入 ${formatWorkProgress(summary.saved, summary.targetSave || summary.targetFetch)}`
  if (phase === 'derive') return `当前对齐 ${formatWorkProgress(summary.derived, summary.targetDerive)}`
  if (summary.derived || summary.targetDerive) return `当前对齐 ${formatWorkProgress(summary.derived, summary.targetDerive)}`
  if (summary.saved || summary.targetSave) return `当前写入 ${formatWorkProgress(summary.saved, summary.targetSave)}`
  if (summary.fetched || summary.targetFetch) return `当前拉取 ${formatWorkProgress(summary.fetched, summary.targetFetch)}`
  return '同步中'
}

export function secondaryText(summary: SyncProgressSummary) {
  if (summary.total <= 1) return ''
  if (summary.active > 0) return `活跃 ${formatInteger(summary.active)}`
  return ''
}

function currentJobMessage(job: SyncJob | null) {
  if (!job || job.status !== 'running') return ''
  return String(job.message || '').trim()
}
