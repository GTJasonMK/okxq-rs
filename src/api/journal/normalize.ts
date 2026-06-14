import type * as T from '@/types/journal'
import {
  arrayRecords,
  arrayValue,
  isRecord,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'

type AnyRecord = Record<string, unknown>

export function normalizeEntry(raw: AnyRecord): T.JournalEntry {
  const entryId = stringValue(raw.entry_id)
  return {
    id: entryId,
    entry_id: entryId,
    title: stringValue(raw.title, '未命名日志'),
    content: stringValue(raw.content),
    mode: normalizeMode(raw.mode),
    inst_id: stringValue(raw.inst_id),
    inst_type: stringValue(raw.inst_type, 'SPOT'),
    trade_ids: normalizeStringList(raw.trade_ids),
    order_ids: normalizeStringList(raw.order_ids),
    tags: normalizeStringList(raw.tags),
    strategy_id: stringValue(raw.strategy_id),
    strategy_name: stringValue(raw.strategy_name),
    rating: normalizeRating(raw.rating),
    emotion: stringValue(raw.emotion),
    screenshots: normalizeStringList(raw.screenshots),
    pnl_snapshot: numberValue(raw.pnl_snapshot),
    metadata: isRecord(raw.metadata) ? raw.metadata : {},
    created_at: normalizeDateString(raw.created_at),
    updated_at: normalizeDateString(raw.updated_at),
  }
}

export function normalizeEntryPayload(data: Partial<T.JournalEntry>): Record<string, unknown> {
  const raw = data as Partial<T.JournalEntry> & Record<string, unknown>
  const payload: Record<string, unknown> = {}
  assignStringIfPresent(payload, 'entry_id', raw.entry_id, true)
  assignStringIfPresent(payload, 'title', raw.title, true)
  assignStringIfPresent(payload, 'content', raw.content)
  assignModeIfPresent(payload, raw.mode)
  assignStringIfPresent(payload, 'inst_id', raw.inst_id, true)
  assignStringIfPresent(payload, 'inst_type', raw.inst_type, true)
  assignStringIfPresent(payload, 'strategy_id', raw.strategy_id, true)
  assignStringIfPresent(payload, 'strategy_name', raw.strategy_name, true)
  assignRatingIfPresent(payload, raw.rating)
  assignStringIfPresent(payload, 'emotion', raw.emotion, true)
  assignNumberIfPresent(payload, 'pnl_snapshot', raw.pnl_snapshot)
  assignStringListIfPresent(payload, 'trade_ids', raw.trade_ids)
  assignStringListIfPresent(payload, 'order_ids', raw.order_ids)
  assignStringListIfPresent(payload, 'tags', raw.tags)
  assignStringListIfPresent(payload, 'screenshots', raw.screenshots)
  if (isRecord(raw.metadata)) payload.metadata = raw.metadata
  assignStringIfPresent(payload, 'created_at', raw.created_at, true)
  return payload
}

export function normalizeTag(raw: AnyRecord): T.JournalTag {
  return {
    tag: stringValue(raw.tag),
    usage_count: numberValue(raw.usage_count),
    color: stringValue(raw.color),
    created_at: normalizeDateString(raw.created_at, ''),
  }
}

export function normalizeStats(raw: unknown): T.JournalStats {
  const item = recordFrom(raw)
  return {
    total_entries: numberValue(item.total_entries),
    group_by: stringValue(item.group_by, 'tag'),
    groups: arrayRecords(item.groups).map(normalizeStatsGroup),
  }
}

function normalizeStatsGroup(raw: AnyRecord): T.JournalStatsGroup {
  return {
    key: stringValue(raw.key, '未知'),
    count: numberValue(raw.count),
    total_pnl: numberValue(raw.total_pnl),
    win_rate: numberValue(raw.win_rate),
    avg_rating: numberValue(raw.avg_rating),
  }
}

function normalizeStringList(value: unknown): string[] {
  return arrayValue(value)
    .map(item => stringValue(item).trim())
    .filter(Boolean)
}

function normalizeRating(value: unknown): number {
  const rating = Math.round(numberValue(value, Number.NaN))
  if (!Number.isFinite(rating)) return 0
  return Math.min(5, Math.max(0, rating))
}

function normalizeMode(value: unknown): T.JournalEntry['mode'] {
  return stringValue(value, 'simulated').trim().toLowerCase() === 'live' ? 'live' : 'simulated'
}

function normalizeDateString(value: unknown, defaultValue = ''): string {
  const text = stringValue(value).trim()
  return text || defaultValue
}

function assignStringIfPresent(
  target: Record<string, unknown>,
  key: string,
  value: unknown,
  trim = false,
) {
  if (typeof value !== 'string') return
  target[key] = trim ? value.trim() : value
}

function assignNumberIfPresent(target: Record<string, unknown>, key: string, value: unknown) {
  if (typeof value === 'number' && Number.isFinite(value)) target[key] = value
}

function assignRatingIfPresent(target: Record<string, unknown>, value: unknown) {
  if (typeof value === 'number' && Number.isFinite(value)) target.rating = normalizeRating(value)
}

function assignModeIfPresent(target: Record<string, unknown>, value: unknown) {
  if (typeof value === 'string') target.mode = normalizeMode(value)
}

function assignStringListIfPresent(target: Record<string, unknown>, key: string, value: unknown) {
  if (Array.isArray(value)) target[key] = normalizeStringList(value)
}
