import type {
  InstType,
  SyncJob,
  SyncRecord,
  Timeframe,
  WatchedSymbolSyncPlan,
} from '@/types'
import type { PlanRow, WatchedRow } from '@/types/dataCenter'
import { timeframeOrder } from '@/utils/syncPlans'
import {
  formatJobFailure,
  formatJobStatus,
  newestJob,
  syncJobTargetTimeframes,
} from '@/utils/dataCenter/syncJobs'
import { effectivePlansForRow } from './effective'
import { isFullArchivePlan, planPolicyLabel } from './labels'
import { recordStatus, syncRecordCoverageLabel } from './records'

export type SyncRecordScopeIndex = Map<string, SyncRecord[]>

export function buildSyncRecordScopeIndex(records: SyncRecord[]): SyncRecordScopeIndex {
  const index: SyncRecordScopeIndex = new Map()
  for (const record of records) {
    appendSyncRecordScope(index, record)
  }
  return index
}

export function syncRecordsForScope(
  index: SyncRecordScopeIndex,
  instId: string,
  instType: InstType,
) {
  return index.get(syncRecordScopeKey(instId, instType)) ?? []
}

export function replaceSyncRecordScopes(
  index: SyncRecordScopeIndex,
  records: SyncRecord[],
  scopeKeys: Set<string>,
): SyncRecordScopeIndex {
  if (scopeKeys.size === 0) return buildSyncRecordScopeIndex(records)
  const nextIndex: SyncRecordScopeIndex = new Map(index)
  replaceSyncRecordScopesInPlace(nextIndex, records, scopeKeys)
  return nextIndex
}

export function replaceSyncRecordScopesInPlace(
  index: SyncRecordScopeIndex,
  records: SyncRecord[],
  scopeKeys: Set<string>,
): SyncRecordScopeIndex {
  if (scopeKeys.size === 0) {
    index.clear()
  } else {
    for (const key of scopeKeys) index.delete(key)
  }
  for (const record of records) {
    appendSyncRecordScope(index, record)
  }
  return index
}

export function syncRecordScopeKey(instId: string, instType: string) {
  return `${instType.trim().toUpperCase()}:${instId.trim().toUpperCase()}`
}

function appendSyncRecordScope(index: SyncRecordScopeIndex, record: SyncRecord) {
  const key = syncRecordScopeKey(record.inst_id, record.inst_type)
  const items = index.get(key)
  if (items) {
    items.push(record)
  } else {
    index.set(key, [record])
  }
}

export function buildPlanRows(
  row: WatchedRow,
  instType: InstType,
  records: SyncRecord[],
  enabledPlans: WatchedSymbolSyncPlan[],
): PlanRow[] {
  const instId = instType === 'SPOT' ? row.spot_inst_id : row.swap_inst_id
  const scopeRecords = records.filter(record => record.inst_id === instId && record.inst_type === instType)
  return buildPlanRowsFromScopeRecords(row, instType, scopeRecords, enabledPlans)
}

export function buildPlanRowsFromScopeRecords(
  row: WatchedRow,
  instType: InstType,
  records: SyncRecord[],
  enabledPlans: WatchedSymbolSyncPlan[],
): PlanRow[] {
  const instId = instType === 'SPOT' ? row.spot_inst_id : row.swap_inst_id
  const plannedRows = effectivePlansForRow(row, enabledPlans).map(plan => ({
    timeframe: plan.timeframe,
    policyLabel: planPolicyLabel(plan, row.archive_all_history),
    fullArchive: isFullArchivePlan(plan, row.archive_all_history),
  }))
  const recordByTimeframe = recordsByTimeframe(records)
  const jobsByTimeframe = planJobsByTimeframe(row.jobs, instId, instType)
  const timeframeRows = new Map<PlanRow['timeframe'], {
    timeframe: PlanRow['timeframe']
    policyLabel: string
    fullArchive: boolean
  }>()
  for (const plan of plannedRows) {
    timeframeRows.set(plan.timeframe, plan)
  }
  for (const record of records) {
    if (!timeframeRows.has(record.timeframe)) {
      timeframeRows.set(record.timeframe, {
        timeframe: record.timeframe,
        policyLabel: '库内',
        fullArchive: false,
      })
    }
  }
  return Array.from(timeframeRows.values()).sort((left, right) => (
    timeframeOrder(left.timeframe) - timeframeOrder(right.timeframe)
  )).map(plan => buildPlanRow(
    instId,
    instType,
    recordByTimeframe.get(plan.timeframe),
    jobsByTimeframe.get(plan.timeframe) ?? [],
    plan,
  ))
}

function buildPlanRow(
  instId: string,
  instType: InstType,
  record: SyncRecord | undefined,
  relatedJobs: SyncJob[],
  plan: {
    timeframe: PlanRow['timeframe']
    policyLabel: string
    fullArchive: boolean
  },
): PlanRow {
  const basePlan = {
    inst_id: instId,
    inst_type: instType,
    timeframe: plan.timeframe,
    policyLabel: plan.policyLabel,
    gap_count: record?.gap_count ?? 0,
    start_ts: record?.oldest_timestamp ?? null,
    end_ts: record?.newest_timestamp ?? null,
  }
  const count = record?.candle_count ?? 0
  const activeJob = newestJob(relatedJobs.filter(job => ['queued', 'running'].includes(job.status)))
  if (activeJob) {
    return { ...basePlan, status: activeJob.status as 'queued' | 'running', label: formatJobStatus(activeJob) }
  }
  if (!record || count <= 0) {
    const latestJob = newestJob(relatedJobs)
    if (latestJob?.status === 'failed' && latestJob.error) {
      return { ...basePlan, status: 'failed', label: formatJobFailure(latestJob) }
    }
    return { ...basePlan, status: 'missing', label: '未落库' }
  }
  if (record.history_complete || !plan.fullArchive) {
    return { ...basePlan, status: recordStatus(record), label: syncRecordCoverageLabel(record, false) }
  }
  return { ...basePlan, status: recordStatus(record, true), label: syncRecordCoverageLabel(record, true) }
}

function recordsByTimeframe(records: SyncRecord[]) {
  const byTimeframe = new Map<Timeframe, SyncRecord>()
  for (const record of records) {
    if (!byTimeframe.has(record.timeframe)) {
      byTimeframe.set(record.timeframe, record)
    }
  }
  return byTimeframe
}

function planJobsByTimeframe(jobs: SyncJob[], instId: string, instType: InstType) {
  const byTimeframe = new Map<Timeframe, SyncJob[]>()
  for (const job of jobs) {
    if (job.inst_id !== instId || job.inst_type !== instType) continue
    const targets = new Set<Timeframe>([job.timeframe, ...syncJobTargetTimeframes(job)])
    for (const timeframe of targets) {
      const items = byTimeframe.get(timeframe)
      if (items) {
        items.push(job)
      } else {
        byTimeframe.set(timeframe, [job])
      }
    }
  }
  return byTimeframe
}
