import type { SyncJob, SyncRecord } from '@/types'
import { parseDateTimeMs, syncJobSortTime } from '@/utils/syncProgress'

export type SyncRecordTargetIndex = Map<string, string | null>

export function syncJobTargetTimeframes(job: SyncJob) {
  const targets = job.target_timeframes?.length ? job.target_timeframes : [job.timeframe]
  return Array.from(new Set(targets.filter(Boolean)))
}

export function syncJobSupersededByRecords(
  job: SyncJob,
  records: SyncRecord[],
  targets = syncJobTargetTimeframes(job),
) {
  return syncJobSupersededByRecordIndex(job, buildSyncRecordTargetIndex(records), targets)
}

export function buildSyncRecordTargetIndex(records: SyncRecord[]): SyncRecordTargetIndex {
  const index: SyncRecordTargetIndex = new Map()
  for (const record of records) {
    if (record.candle_count <= 0) continue
    const key = syncRecordTargetKey(record.inst_id, record.inst_type, record.timeframe)
    if (!index.has(key)) {
      index.set(key, record.last_sync_time ?? null)
    }
  }
  return index
}

export function syncJobSupersededByRecordIndex(
  job: SyncJob,
  recordIndex: SyncRecordTargetIndex,
  targets = syncJobTargetTimeframes(job),
) {
  const jobTime = syncJobSortTime(job)
  if (jobTime <= 0 || targets.length === 0) return false
  for (const timeframe of targets) {
    const lastSyncTime = recordIndex.get(syncRecordTargetKey(job.inst_id, job.inst_type, timeframe))
    const recordTime = lastSyncTime ? parseDateTimeMs(lastSyncTime) : 0
    if (recordTime < jobTime) return false
  }
  return true
}

function syncRecordTargetKey(instId: string, instType: string, timeframe: string) {
  return `${instType}\u0000${instId}\u0000${timeframe}`
}
