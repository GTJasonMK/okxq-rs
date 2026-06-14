import type {
  SyncRecord,
} from '@/types'
import type { PlanRow } from '@/types/dataCenter'
import { formatCount, formatOptionalDateTime } from '@/utils/dataCenter/format'

export function recordStatus(record: SyncRecord, partial = false): PlanRow['status'] {
  if ((record.gap_count ?? 0) > 0) return 'failed'
  return partial ? 'partial' : 'ok'
}

export function syncRecordCoverageLabel(record: SyncRecord, partial = false) {
  const range = syncRecordRangeLabel(record)
  const gapCount = record.gap_count ?? 0
  if (gapCount > 0) return `${range} · 缺失 ${formatCount(gapCount)}`
  if (partial) return `${range} · 未全量`
  return `${range} · 无缺失`
}

function syncRecordRangeLabel(record: SyncRecord) {
  const oldest = formatOptionalDateTime(record.oldest_time ?? record.oldest_timestamp)
  const newest = formatOptionalDateTime(record.newest_time ?? record.newest_timestamp)
  if (oldest && newest) return `${oldest} 至 ${newest}`
  if (newest) return `最新 ${newest}`
  if (oldest) return `起始 ${oldest}`
  return '范围 --'
}
