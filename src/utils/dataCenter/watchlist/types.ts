import type {
  SyncJob,
  WatchedSymbolSyncPlan,
} from '@/types'

export type WatchRuleFormState = {
  syncSpot: boolean
  syncSwap: boolean
  archiveAll: boolean
  autoSync: boolean
  syncDays: number
  syncPlans: WatchedSymbolSyncPlan[]
}

export type SyncTaskSubmissionResult = {
  sync_jobs?: SyncJob[]
  started_count?: number
  reused_count?: number
  exact_gap_jobs?: number
  rule_jobs?: number
}
