import { formatWorkProgress } from '@/utils/syncProgress/format'
import { bounded, clampPercent, nonNegative, scaleProgress } from '@/utils/syncProgress/numbers'
import type { SyncPhase, SyncProgressSegment, SyncProgressSummary } from '@/utils/syncProgress/types'

export function buildSegments(summary: SyncProgressSummary, phase: SyncPhase): SyncProgressSegment[] {
  if (summary.total > 0 && phase === 'completed') {
    return [{
      key: 'fetch',
      label: '完成',
      done: 100,
      total: 100,
      progress: 100,
      weight: 100,
      active: false,
      text: '100%',
    }]
  }

  if (summary.active > 0 && ['fetch', 'save', 'derive', 'running', 'queued'].includes(phase)) {
    return [{
      key: phase === 'save' || phase === 'derive' ? phase : 'fetch',
      label: currentPhaseSegmentLabel(phase),
      done: summary.progress,
      total: 100,
      progress: summary.progress,
      weight: 100,
      active: phase !== 'queued',
      text: `${summary.progress}%`,
    }]
  }

  const items: SyncProgressSegment[] = [
    progressSegment('fetch', '拉取', summary.fetched, summary.targetFetch, 50, phase),
    progressSegment('save', '落库', summary.saved, summary.targetSave || summary.targetFetch, 30, phase),
    progressSegment('derive', '对齐', summary.derived, summary.targetDerive, 20, phase),
  ].filter(segment => segment.total > 0 || segment.done > 0)

  if (
    items.length > 0 &&
    (phase === 'failed' || phase === 'cancelled') &&
    summary.progress > 0 &&
    items.every(segment => segment.progress === 0)
  ) {
    return [{
      key: 'fetch',
      label: '任务',
      done: summary.progress,
      total: 100,
      progress: summary.progress,
      weight: 100,
      active: false,
      text: `${summary.progress}%`,
    }]
  }

  if (items.length > 0) {
    return items.map(segment => ({
      ...segment,
      progress: visualSegmentProgress(segment, summary.progress, phase),
    }))
  }
  if (summary.total > 0) {
    return [{
      key: 'fetch',
      label: '任务',
      done: summary.progress,
      total: 100,
      progress: summary.progress,
      weight: 100,
      active: summary.active > 0,
      text: `${summary.progress}%`,
    }]
  }
  return []
}

function progressSegment(
  key: SyncProgressSegment['key'],
  label: string,
  done: number,
  total: number,
  weight: number,
  phase: SyncPhase,
): SyncProgressSegment {
  const safeTotal = nonNegative(total)
  const safeDone = bounded(nonNegative(done), safeTotal || nonNegative(done))
  const progress = safeTotal > 0 ? clampPercent(Math.round((safeDone * 100) / safeTotal)) : 0
  return {
    key,
    label,
    done: safeDone,
    total: safeTotal,
    progress,
    weight,
    active: phase === key,
    text: formatWorkProgress(safeDone, safeTotal),
  }
}

function visualSegmentProgress(segment: SyncProgressSegment, progress: number, phase: SyncPhase) {
  if (phase === 'completed') return 100
  if (phase === 'failed' || phase === 'cancelled' || phase === 'queued') return segment.progress
  if (segment.key === 'fetch') {
    if (progress >= 68 || phase === 'save' || phase === 'derive') return 100
    return Math.max(segment.progress, scaleProgress(progress, 0, 68))
  }
  if (segment.key === 'save') {
    if (progress >= 88 || phase === 'derive') return 100
    if (phase === 'save' || progress >= 68) return Math.max(segment.progress, scaleProgress(progress, 20, 68))
    return segment.progress
  }
  if (segment.key === 'derive') {
    if (phase === 'derive' || progress >= 88) return Math.max(segment.progress, scaleProgress(progress, 88, 98))
    return segment.progress
  }
  return segment.progress
}

function currentPhaseSegmentLabel(phase: SyncPhase) {
  if (phase === 'save') return '写入'
  if (phase === 'derive') return '对齐'
  if (phase === 'queued') return '等待'
  return '当前'
}
