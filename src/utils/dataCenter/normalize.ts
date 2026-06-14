import type { InstType } from '@/types'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  isRecord,
  numberValue,
  recordFrom,
  stringValue,
} from '@/api/normalize'

export function nullableTimestampValue(value: unknown): string | number | null {
  if (value === null || value === undefined) return null
  return typeof value === 'string' || typeof value === 'number' ? value : null
}

export {
  arrayRecords,
  arrayValue,
  booleanValue,
  numberValue,
  recordFrom,
  stringValue,
}

export function timestampFromValue(value: unknown, fallback?: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) return value
  const text = stringValue(fallback).trim()
  if (!text) return null
  const parsed = Date.parse(text)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}

export function ratioValue(value: unknown) {
  const ratio = numberValue(value)
  if (ratio <= 0) return 0
  if (ratio >= 1) return 1
  return ratio
}

export function formatRatio(value: number) {
  const ratio = ratioValue(value)
  const percent = ratio * 100
  if (percent === 0 || percent === 100) return `${percent.toFixed(0)}%`
  return `${percent.toFixed(1)}%`
}

export function normalizeStringList(value: unknown): string[] {
  return arrayValue(value)
    .filter((item): item is string => typeof item === 'string')
    .map(item => item.trim())
    .filter(Boolean)
}

export function normalizeNumberRecord(value: unknown): Record<string, number> {
  if (!isRecord(value)) return {}
  return Object.entries(value).reduce<Record<string, number>>((acc, [key, item]) => {
    acc[key] = numberValue(item)
    return acc
  }, {})
}

export function normalizeInstType(value: unknown): InstType | '' {
  const raw = stringValue(value).trim().toUpperCase()
  if (raw === 'SPOT' || raw === 'SWAP' || raw === 'FUTURES') return raw
  return ''
}

export function toNullableString(value: unknown): string | null {
  const text = stringValue(value).trim()
  return text || null
}

export function isValidTimestamp(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

export function normalizeInputSymbol(value: string) {
  let normalized = value.trim().toUpperCase()
  if (!normalized) return ''
  if (normalized.endsWith('-SWAP')) normalized = normalized.slice(0, -5)
  if (!normalized.includes('-')) normalized = `${normalized}-USDT`
  return normalized
}
