import { apiGet } from '../client'
import type * as T from '@/types/live-strategy'
import {
  arrayValue,
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  stringValue,
  timestampString,
} from '../normalize'
import { normalizeInstType, normalizeTimeframe } from '../marketNormalize'
import { tradingMode } from './shared'

export function fetchLiveExecutionPlans(params: { limit?: number; mode?: T.LiveStrategyStatus['mode']; run_id?: string } = {}) {
  const query: Record<string, string | number> = {}
  if (params.limit) query.limit = params.limit
  if (params.mode) query.mode = params.mode
  if (params.run_id) query.run_id = params.run_id
  return apiGet<unknown>('/api/live/execution-plans', query)
    .then(data => arrayValue<Record<string, unknown>>(data).map(normalizeExecutionPlan))
}

function normalizeExecutionPlan(raw: Record<string, unknown>): T.LiveExecutionPlan {
  return {
    id: numberValue(raw.id),
    plan_key: stringValue(raw.plan_key),
    strategy_id: stringValue(raw.strategy_id),
    strategy_name: stringValue(raw.strategy_name),
    mode: tradingMode(raw.mode),
    entry_run_id: stringValue(raw.entry_run_id),
    exit_run_id: stringValue(raw.exit_run_id),
    symbol: stringValue(raw.symbol),
    inst_id: stringValue(raw.inst_id) || stringValue(raw.symbol),
    inst_type: normalizeInstType(raw.inst_type),
    timeframe: normalizeTimeframe(stringValue(raw.timeframe)) as T.LiveExecutionPlan['timeframe'],
    entry_order_id: stringValue(raw.entry_order_id),
    entry_client_order_id: stringValue(raw.entry_client_order_id),
    entry_timestamp: positiveTimestamp(raw.entry_timestamp),
    entry_side: stringValue(raw.entry_side),
    entry_price: nullableNumberValue(raw.entry_price),
    close_side: stringValue(raw.close_side),
    planned_exit_time: positiveTimestamp(raw.planned_exit_time),
    planned_exit_reason: stringValue(raw.planned_exit_reason),
    planned_exit_contract: stringValue(raw.planned_exit_contract),
    status: stringValue(raw.status),
    exit_order_id: stringValue(raw.exit_order_id),
    exit_client_order_id: stringValue(raw.exit_client_order_id),
    attempt_count: numberValue(raw.attempt_count),
    next_attempt_at: positiveTimestamp(raw.next_attempt_at),
    last_error: stringValue(raw.last_error),
    created_at: timestampString(raw.created_at),
    updated_at: timestampString(raw.updated_at),
  }
}

function positiveTimestamp(value: unknown): number | null {
  const timestamp = nullableTimestampNumber(value)
  return timestamp !== null && timestamp > 0 ? timestamp : null
}
