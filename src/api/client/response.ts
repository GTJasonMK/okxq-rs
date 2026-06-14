import { ApiError } from '@/types/api'
import { describeError } from '@/utils/logger'

export function unwrapResponse<T>(raw: unknown): T {
  if (raw === null || raw === undefined) {
    throw new ApiError('后端返回空数据', 0)
  }
  if (!isRecord(raw)) return raw as T

  const obj = raw
  const code = typeof obj.code === 'number' && Number.isFinite(obj.code) ? obj.code : null
  const hasCodeEnvelope = code !== null
  if (code !== null && code !== 0) {
    throw new ApiError(apiErrorMessage(raw), code, raw)
  }
  if (hasCodeEnvelope && 'data' in obj && obj.data !== undefined) {
    return obj.data as T
  }
  return raw as T
}

function apiErrorMessage(raw: unknown): string {
  const detail = findDetail(raw)
  if (detail && typeof detail === 'object') {
    const base = describeError(detail) || describeError(raw)
    const blockers = blockingIds(detail, raw)
    if (blockers.length > 0) return `${base}：${blockers.join('、')}`
    return base
  }
  const base = describeError(detail ?? raw)
  const blockers = blockingIds(raw)
  if (blockers.length > 0) return `${base}：${blockers.join('、')}`
  return base
}

function findDetail(value: unknown): unknown {
  if (!isRecord(value)) return undefined
  return value.detail ?? value.message ?? value.msg ?? value.error ?? value.data
}

function blockingIds(...values: unknown[]): string[] {
  const keys = ['blocking_dataset_ids', 'blocking_training_run_ids', 'blocking_session_ids']
  const ids: string[] = []
  for (const value of values) {
    if (!isRecord(value)) continue
    for (const source of [value, value.data]) {
      if (!isRecord(source)) continue
      for (const key of keys) {
        const item = source[key]
        if (Array.isArray(item)) {
          ids.push(...item.map(entry => String(entry)).filter(Boolean))
        }
      }
    }
  }
  return Array.from(new Set(ids))
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
