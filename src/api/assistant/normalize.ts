import type {
  ChatMessage,
  ChatSession,
  LevelSnapshot,
  OrderDraft,
  PatrolConfig,
  PatrolRun,
} from '@/types'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  isRecord,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'

type AnyRecord = Record<string, unknown>

export interface AssistantStatus {
  enabled: boolean
  configured: boolean
  provider_name: string
  model: string
  runtime: string
}

export interface AssistantToolCapability {
  name: string
  description: string
}

export function normalizeAssistantStatus(raw: unknown): AssistantStatus {
  const item = recordFrom(raw)
  return {
    enabled: booleanValue(item.enabled),
    configured: booleanValue(item.configured),
    provider_name: stringValue(item.provider_name),
    model: stringValue(item.model),
    runtime: stringValue(item.runtime),
  }
}

export function normalizeToolCapability(raw: AnyRecord): AssistantToolCapability {
  return {
    name: stringValue(raw.name),
    description: stringValue(raw.description),
  }
}

export function normalizeSessionRequest(raw: Record<string, unknown>): Record<string, unknown> {
  const request: Record<string, unknown> = {
    title: stringValue(raw.title).trim(),
    inst_id: stringValue(raw.inst_id).trim(),
    inst_type: stringValue(raw.inst_type, 'SPOT').trim(),
    metadata: isRecord(raw.metadata) ? raw.metadata : {},
  }
  if (raw.mode !== undefined) {
    request.mode = normalizeMode(raw.mode)
  }
  return request
}

export function normalizeSession(raw: AnyRecord): ChatSession {
  return {
    id: stringValue(raw.id),
    session_id: stringValue(raw.session_id),
    title: stringValue(raw.title, '未命名会话'),
    mode: normalizeMode(raw.mode),
    created_at: normalizeDateString(raw.created_at),
  }
}

export function normalizeSessionDetail(raw: unknown) {
  const detail = recordFrom(raw)
  return {
    session: normalizeSession(recordFrom(detail.session)),
    messages: arrayRecords(detail.messages).map(normalizeMessage),
    steps: arrayRecords(detail.steps).map(normalizeStep),
    order_drafts: arrayRecords(detail.order_drafts).map(normalizeOrderDraft),
    level_snapshots: arrayRecords(detail.level_snapshots).map(normalizeLevelSnapshot),
  }
}

export function normalizeChatResponse(raw: unknown) {
  const payload = recordFrom(raw)
  const detail = recordFrom(payload.detail)
  return {
    message: normalizeMessage(recordFrom(payload.assistant_message)),
    messages: arrayRecords(detail.messages).map(normalizeMessage),
    session: normalizeSession(recordFrom(payload.session)),
  }
}

export function normalizeOrderDraft(raw: AnyRecord): OrderDraft {
  return {
    id: stringValue(raw.id),
    draft_id: stringValue(raw.draft_id),
    session_id: stringValue(raw.session_id),
    inst_id: stringValue(raw.inst_id),
    mode: normalizeMode(raw.mode),
    side: stringValue(raw.side),
    order_type: stringValue(raw.order_type, 'market').toLowerCase(),
    size: stringValue(raw.size),
    price: stringValue(raw.price),
    status: stringValue(raw.status, 'draft').toLowerCase(),
    created_at: normalizeDateString(raw.created_at),
    updated_at: normalizeDateString(raw.updated_at),
  }
}

export function normalizePatrolStatus(raw: unknown) {
  const item = recordFrom(raw)
  return {
    running: booleanValue(item.running),
    current_phase: stringValue(item.current_phase, 'idle'),
    last_run_started_at: normalizeOptionalDateString(item.last_run_started_at),
    last_run_finished_at: normalizeOptionalDateString(item.last_run_finished_at),
    last_run_summary: isRecord(item.last_run_summary) ? item.last_run_summary : {},
    last_error: stringValue(item.last_error),
    recent_events: arrayValue(item.recent_events),
    settings: normalizePatrolConfig(item.settings),
  }
}

export function normalizePatrolConfig(raw: unknown): PatrolConfig {
  const item = recordFrom(raw)
  const intervalSeconds = numberValue(item.interval_seconds, 300)
  return {
    enabled: booleanValue(item.enabled),
    interval_seconds: intervalSeconds,
    interval_minutes: intervalSeconds / 60,
    symbols: normalizeStringList(item.symbols),
    scan_limit: numberValue(item.scan_limit, 24),
    candidate_limit: numberValue(item.candidate_limit, 3),
    inst_type: stringValue(item.inst_type, 'SWAP'),
    timeframes: normalizeStringList(item.timeframes),
    candles_limit: numberValue(item.candles_limit, 240),
    recent_trade_limit: numberValue(item.recent_trade_limit, 40),
    orderbook_depth: numberValue(item.orderbook_depth, 30),
    mode: normalizeMode(item.mode),
    min_priority_score: numberValue(item.min_priority_score, 55),
    notification_cooldown_seconds: numberValue(item.notification_cooldown_seconds, 900),
  }
}

export function normalizePatrolConfigRequest(raw: Record<string, unknown>): Record<string, unknown> {
  return {
    enabled: booleanValue(raw.enabled),
    interval_seconds: numberValue(raw.interval_seconds, 300),
    symbols: normalizeStringList(raw.symbols),
    scan_limit: numberValue(raw.scan_limit, 24),
    candidate_limit: numberValue(raw.candidate_limit, 3),
    inst_type: stringValue(raw.inst_type, 'SWAP').trim(),
    timeframes: normalizeStringList(raw.timeframes),
    candles_limit: numberValue(raw.candles_limit, 240),
    recent_trade_limit: numberValue(raw.recent_trade_limit, 40),
    orderbook_depth: numberValue(raw.orderbook_depth, 30),
    mode: normalizeMode(raw.mode),
    min_priority_score: numberValue(raw.min_priority_score, 55),
    notification_cooldown_seconds: numberValue(raw.notification_cooldown_seconds, 900),
  }
}

export function normalizePatrolConfigUpdate(raw: unknown) {
  const item = recordFrom(raw)
  return {
    settings: normalizePatrolConfig(item.settings),
    status: normalizePatrolStatus(item.status),
  }
}

export function normalizePatrolRun(raw: unknown): PatrolRun {
  const item = recordFrom(raw)
  return {
    run_id: stringValue(item.run_id),
    status: stringValue(item.status, 'completed'),
    candidates: arrayValue(item.candidates),
    summary: isRecord(item.summary) ? item.summary : {},
    started_at: normalizeOptionalDateString(item.started_at),
    finished_at: normalizeOptionalDateString(item.finished_at),
  }
}

function normalizeMessage(raw: AnyRecord): ChatMessage {
  const role = stringValue(raw.role, 'assistant').trim().toLowerCase()
  return {
    id: stringValue(raw.id),
    message_id: stringValue(raw.message_id),
    session_id: stringValue(raw.session_id),
    role: role === 'user' || role === 'system' ? role : 'assistant',
    content: stringValue(raw.content),
    created_at: normalizeDateString(raw.created_at),
  }
}

function normalizeStep(raw: AnyRecord) {
  return {
    id: stringValue(raw.id),
    step_id: stringValue(raw.step_id),
    session_id: stringValue(raw.session_id),
    step_type: stringValue(raw.step_type),
    title: stringValue(raw.title),
    status: stringValue(raw.status, 'completed'),
    created_at: normalizeDateString(raw.created_at),
  }
}

function normalizeLevelSnapshot(raw: AnyRecord): LevelSnapshot {
  return {
    id: stringValue(raw.id),
    snapshot_id: stringValue(raw.snapshot_id),
    session_id: stringValue(raw.session_id),
    inst_id: stringValue(raw.inst_id),
    mode: normalizeMode(raw.mode),
    timeframes: arrayValue(raw.timeframes),
    supports: arrayValue(raw.supports),
    resistances: arrayValue(raw.resistances),
    invalidation_levels: arrayValue(raw.invalidation_levels),
    chart_annotations: arrayValue(raw.chart_annotations),
    summary: isRecord(raw.summary) ? raw.summary : {},
    metadata: isRecord(raw.metadata) ? raw.metadata : {},
    created_at: normalizeDateString(raw.created_at),
  }
}

function normalizeStringList(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(item => stringValue(item).trim()).filter(Boolean)
  return []
}

function normalizeMode(value: unknown): 'simulated' | 'live' {
  return stringValue(value, 'simulated').trim().toLowerCase() === 'live' ? 'live' : 'simulated'
}

function normalizeDateString(value: unknown, defaultValue = new Date().toISOString()): string {
  const text = stringValue(value).trim()
  return text || defaultValue
}

function normalizeOptionalDateString(value: unknown): string | null {
  const text = stringValue(value).trim()
  if (!text) return null
  return normalizeDateString(value, '') || null
}
