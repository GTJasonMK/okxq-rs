import { apiPost } from '../../client'
import type * as T from '@/types/live-strategy'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  isRecord,
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  recordFrom,
  stringValue,
  timestampNumber,
} from '../../normalize'
import {
  normalizeInstType,
  normalizeTimeframe,
} from '../../marketNormalize'
import {
  normalizeDecisionDiagnosticsPayload,
} from '../payload'

export function fetchDecisionDiagnostics(data: Record<string, unknown>) {
  return apiPost<unknown>('/api/live/decision-diagnostics', normalizeDecisionDiagnosticsPayload(data))
    .then(normalizeDecisionDiagnostics)
}

function normalizeDecisionDiagnostics(raw: unknown): T.LiveDecisionDiagnostics {
  const item = recordFrom(raw)
  return {
    strategy_id: stringValue(item.strategy_id),
    strategy_name: stringValue(item.strategy_name),
    symbol: stringValue(item.symbol),
    inst_type: normalizeInstType(item.inst_type) as T.LiveDecisionDiagnostics['inst_type'],
    timeframe: normalizeTimeframe(stringValue(item.timeframe)) as T.LiveDecisionDiagnostics['timeframe'],
    summary: stringValue(item.summary),
    candle_count: nullableNumberValue(item.candle_count),
    realtime_candle_applied: booleanValue(item.realtime_candle_applied),
    decision_protocol: stringValue(item.decision_protocol),
    actions: arrayRecords(item.actions).map(normalizeLiveStrategyAction),
    action_summary: normalizeActionSummary(item.action_summary),
    execution_logs: arrayRecords(item.execution_logs).map(normalizeDecisionExecutionLog),
    selected_symbols: arrayValue<unknown>(item.selected_symbols)
      .map(value => stringValue(value).trim())
      .filter(Boolean),
    blocked_by: arrayValue<unknown>(item.blocked_by).map(value => stringValue(value)).filter(Boolean),
    execution_decision: normalizeDecisionExecutionPreview(item.execution_decision),
    raw: item,
  }
}

function normalizeLiveStrategyAction(raw: Record<string, unknown>): T.LiveStrategyAction {
  return {
    action: stringValue(raw.action),
    symbol: stringValue(raw.symbol).toUpperCase(),
    side: stringValue(raw.side),
    order_type: stringValue(raw.order_type),
    price: nullableNumberValue(raw.price),
    reference_price: nullableNumberValue(raw.reference_price),
    trigger_price: nullableNumberValue(raw.trigger_price),
    price_source: stringValue(raw.price_source),
    reason: stringValue(raw.reason),
    strength: nullableNumberValue(raw.strength),
    timestamp: timestampNumber(raw.timestamp),
    position_size: nullableNumberValue(raw.position_size),
    exchange_size: stringValue(raw.exchange_size),
    order_side: stringValue(raw.order_side),
    close_side: stringValue(raw.close_side),
    planned_exit_time: nullableTimestampValue(raw.planned_exit_time),
    planned_exit_reason: stringValue(raw.planned_exit_reason),
    planned_exit_contract: stringValue(raw.planned_exit_contract),
    order_id: stringValue(raw.order_id),
    client_order_id: stringValue(raw.client_order_id),
    new_size: stringValue(raw.new_size),
    new_price: stringValue(raw.new_price),
    request_id: stringValue(raw.request_id),
    cancel_on_fail: booleanValue(raw.cancel_on_fail),
    target_order_kind: stringValue(raw.target_order_kind),
    target_order_type: stringValue(raw.target_order_type),
    source_index: nullableIntegerValue(raw.source_index),
    source_time: nullableTimestampValue(raw.source_time),
    feature_bar_time: nullableTimestampValue(raw.feature_bar_time),
    entry_time: nullableTimestampValue(raw.entry_time),
    planned_hold_bars: nullableIntegerValue(raw.planned_hold_bars),
    hold_bars: nullableIntegerValue(raw.hold_bars),
    layer_id: stringValue(raw.layer_id),
    family: stringValue(raw.family),
    action_timeframe: normalizeTimeframe(stringValue(raw.timeframe)),
    candidate_source: stringValue(raw.candidate_source),
    candidate_entry_price: nullableNumberValue(raw.candidate_entry_price),
    raw,
  }
}

function normalizeActionSummary(raw: unknown): T.LiveDecisionActionSummary {
  const item = recordFrom(raw)
  return {
    open_position: integerValue(item.open_position),
    close_position: integerValue(item.close_position),
    place_risk_order: integerValue(item.place_risk_order),
    cancel_order: integerValue(item.cancel_order),
    modify_order: integerValue(item.modify_order),
    hold: integerValue(item.hold),
    total: integerValue(item.total),
  }
}

function normalizeDecisionExecutionLog(raw: Record<string, unknown>): T.LiveDecisionExecutionLog {
  return {
    stage: stringValue(raw.stage),
    level: normalizeLogLevel(raw.level),
    message: stringValue(raw.message),
    details: recordFrom(raw.details),
  }
}

function normalizeDecisionExecutionPreview(raw: unknown): T.LiveDecisionExecutionPreview | undefined {
  if (!isRecord(raw)) return undefined
  return {
    verdict: stringValue(raw.verdict) as T.LiveDecisionExecutionPreview['verdict'],
    summary: stringValue(raw.summary),
    executable_intent_count: integerValue(raw.executable_intent_count),
    risk_action_count: integerValue(raw.risk_action_count),
    skipped_action_count: integerValue(raw.skipped_action_count),
    idle_action_count: integerValue(raw.idle_action_count),
    skipped_actions: arrayRecords(raw.skipped_actions).map(normalizeLiveStrategyAction),
    gates: arrayRecords(raw.gates).map(normalizeExecutionGate),
  }
}

function normalizeExecutionGate(raw: Record<string, unknown>): T.LiveExecutionGate {
  return {
    key: stringValue(raw.key),
    label: stringValue(raw.label),
    status: stringValue(raw.status) as T.LiveExecutionGate['status'],
    passed: booleanValue(raw.passed),
    blocking: booleanValue(raw.blocking),
    detail: stringValue(raw.detail),
  }
}

function normalizeLogLevel(value: unknown): T.LiveDecisionExecutionLog['level'] {
  const level = stringValue(value).trim().toLowerCase()
  if (level === 'info' || level === 'warn' || level === 'error' || level === 'success') return level
  return ''
}

function integerValue(value: unknown) {
  return Math.max(0, Math.round(numberValue(value, 0)))
}

function nullableIntegerValue(value: unknown) {
  const number = nullableNumberValue(value)
  return number === null ? null : Math.round(number)
}

function nullableTimestampValue(value: unknown) {
  const timestamp = nullableTimestampNumber(value)
  return timestamp !== null && timestamp > 0 ? timestamp : null
}
