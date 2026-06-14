import { apiGet } from '../client'
import type * as T from '@/types/live-strategy'
import {
  arrayValue,
  isRecord,
  numberValue,
  stringValue,
  timestampNumber,
} from '../normalize'

export function fetchLiveExecutionLogs(
  params: { mode?: T.LiveStrategyStatus['mode']; run_id?: string; limit?: number } = {},
) {
  const query: Record<string, string | number> = {}
  if (params.mode) query.mode = params.mode
  if (params.run_id) query.run_id = params.run_id
  if (params.limit) query.limit = params.limit
  return apiGet<unknown>('/api/live/execution-logs', query).then(normalizeLiveExecutionLogs)
}

function normalizeLiveExecutionLogs(raw: unknown): T.LiveExecutionLogEntry[] {
  return arrayValue<unknown>(raw)
    .map(normalizeLiveExecutionLog)
    .filter((item): item is T.LiveExecutionLogEntry => item !== null)
}

function normalizeLiveExecutionLog(raw: unknown): T.LiveExecutionLogEntry | null {
  if (!isRecord(raw)) return null
  const seq = numberValue(raw.seq)
  const message = stringValue(raw.message)
  if (!seq || !message) return null
  return {
    seq,
    run_id: stringValue(raw.run_id),
    mode: stringValue(raw.mode) as T.LiveExecutionLogEntry['mode'],
    strategy_id: stringValue(raw.strategy_id),
    strategy_name: stringValue(raw.strategy_name),
    symbol: stringValue(raw.symbol),
    inst_type: stringValue(raw.inst_type) as T.LiveExecutionLogEntry['inst_type'],
    timeframe: stringValue(raw.timeframe) as T.LiveExecutionLogEntry['timeframe'],
    timestamp_ms: timestampNumber(raw.timestamp_ms),
    time: stringValue(raw.time),
    stage: stringValue(raw.stage),
    level: normalizeLevel(raw.level),
    message,
    details: isRecord(raw.details) ? raw.details : {},
  }
}

function normalizeLevel(value: unknown): T.LiveExecutionLogEntry['level'] {
  const level = stringValue(value).toLowerCase()
  if (level === 'info' || level === 'warn' || level === 'error' || level === 'success') return level
  return ''
}
