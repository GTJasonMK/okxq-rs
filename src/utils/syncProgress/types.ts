export interface SyncProgressSummary {
  total: number
  queued: number
  running: number
  completed: number
  failed: number
  cancelled: number
  active: number
  progress: number
  statusLabel: string
  phaseLabel: string
  primaryText: string
  secondaryText: string
  taskText: string
  segments: SyncProgressSegment[]
  fetched: number
  targetFetch: number
  saved: number
  targetSave: number
  derived: number
  targetDerive: number
  batches: number
  targetBatches: number
  apiCalls: number
}

export interface SyncProgressSegment {
  key: 'fetch' | 'save' | 'derive'
  label: string
  done: number
  total: number
  progress: number
  weight: number
  active: boolean
  text: string
}

export type SyncPhase =
  | 'queued'
  | 'fetch'
  | 'save'
  | 'derive'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'running'
