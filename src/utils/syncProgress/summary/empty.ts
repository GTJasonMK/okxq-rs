import type { SyncProgressSummary } from '@/utils/syncProgress/types'

export function emptySummary(): SyncProgressSummary {
  return {
    total: 0,
    queued: 0,
    running: 0,
    completed: 0,
    failed: 0,
    cancelled: 0,
    active: 0,
    progress: 0,
    statusLabel: '同步任务',
    phaseLabel: '',
    primaryText: '',
    secondaryText: '',
    taskText: '',
    segments: [],
    fetched: 0,
    targetFetch: 0,
    saved: 0,
    targetSave: 0,
    derived: 0,
    targetDerive: 0,
    batches: 0,
    targetBatches: 0,
    apiCalls: 0,
  }
}
