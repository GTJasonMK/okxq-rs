import type { ScannerCondition, ScannerProfile, ScannerResult } from '@/types'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  isRecord,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'

export function normalizeCondition(raw: Record<string, unknown>): ScannerCondition {
  const indicator = stringValue(raw.indicator)
  return {
    ...(raw as unknown as ScannerCondition),
    id: indicator,
    name: stringValue(raw.label, indicator),
    description: stringValue(raw.value_hint),
    params: isRecord(raw.params) ? raw.params : {},
  }
}

export function normalizeProfile(raw: Record<string, unknown>): ScannerProfile {
  const profileId = stringValue(raw.profile_id)
  return {
    ...(raw as unknown as ScannerProfile),
    id: profileId,
    name: stringValue(raw.name, '未命名扫描'),
    conditions: asStringList(raw.conditions),
    symbol_filter: asStringList(raw.symbols),
    inst_type: stringValue(raw.inst_type, '') as ScannerProfile['inst_type'],
    timeframe: stringValue(raw.timeframe, '1H') as ScannerProfile['timeframe'],
    created_at: normalizeDateString(raw.created_at),
  }
}

export function normalizeResult(raw: Record<string, unknown>): ScannerResult {
  const indicatorValues = recordFrom(raw.indicator_values)
  const matchedConditions = asStringList(raw.matched_conditions)
  const price = numberValue(raw.price, numberValue(indicatorValues.price))
  const fallbackScore = matchedConditions.length * 35 + (price > 0 ? 30 : 0)
  const score = clampScore(numberValue(raw.score, fallbackScore))
  const scanTime = stringValue(raw.scan_time) || stringValue(raw.scanned_at)
  const instId = stringValue(raw.inst_id)
  const rowId = idValue(raw.id)
  return {
    id: rowId || [instId, scanTime].filter(Boolean).join('-') || instId,
    profile_id: stringValue(raw.profile_id),
    symbol: instId,
    matched_conditions: matchedConditions,
    score,
    details: { ...indicatorValues, price },
    scanned_at: normalizeDateString(scanTime),
  }
}

export function normalizeScanResponse(data: unknown) {
  const payload = recordFrom(data)
  const results = arrayRecords(payload.results).map(normalizeResult)
  return {
    results,
    scanned: numberValue(payload.scanned, results.length),
    matched: numberValue(payload.matched, results.length),
  }
}

export function normalizeProfilePayload(data: Record<string, unknown>) {
  const arrayConditions = arrayValue(data.conditions)
  const rawConditions = arrayConditions.length > 0 ? arrayConditions : asStringList(data.conditions)
  const conditions = rawConditions.map((condition) => {
    if (isRecord(condition)) return condition
    const indicator = stringValue(condition)
    if (indicator === 'rsi') return { indicator, operator: 'lt', value: 70, params: { period: 14 } }
    if (indicator === 'sma_cross') return { indicator, operator: 'gt', value: 0, params: { fast_period: 5, slow_period: 20 } }
    if (indicator === 'price') return { indicator, operator: 'gt', value: 0, params: {} }
    return { indicator, operator: 'gt', value: 0, params: {} }
  }).filter(condition => stringValue(condition.indicator).length > 0)
  return {
    ...(data.profile_id !== undefined ? { profile_id: stringValue(data.profile_id) } : {}),
    name: stringValue(data.name, '未命名扫描'),
    conditions,
    logic: stringValue(data.logic, 'and'),
    symbols: stringSeries(data.symbols),
    timeframe: stringValue(data.timeframe, '1H'),
    inst_type: stringValue(data.inst_type, ''),
    enabled: booleanValue(data.enabled, true),
    interval_seconds: numberValue(data.interval_seconds, 300),
  }
}

function asStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .map(item => (isRecord(item) ? stringValue(item.indicator) : stringValue(item)))
      .filter(Boolean)
  }
  return []
}

function clampScore(value: number): number {
  return Math.min(100, Math.max(0, value))
}

function normalizeDateString(value: unknown, defaultValue = ''): string {
  const dateText = stringValue(value).trim()
  return dateText || defaultValue
}

function stringSeries(value: unknown): string[] {
  return arrayValue(value).filter(stringFilter)
}

function idValue(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' && Number.isInteger(value)) return value.toString()
  return ''
}

function stringFilter(value: unknown): value is string {
  return typeof value === 'string'
}
