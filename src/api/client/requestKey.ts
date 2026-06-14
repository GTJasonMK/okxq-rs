import type { LocalApiParam, LocalApiRequest } from '@/types/api'

type ApiRequestOptions = {
  params?: Record<string, LocalApiParam>
  body?: unknown
}

export function requestKey(
  method: LocalApiRequest['method'],
  path: string,
  options?: ApiRequestOptions,
): string {
  return stableStringify({
    method,
    path,
    params: options?.params ?? {},
    body: options?.body ?? null,
  })
}

function stableStringify(value: unknown): string {
  return JSON.stringify(sortForStableStringify(value))
}

function sortForStableStringify(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortForStableStringify)
  if (!isRecord(value)) return value
  return Object.keys(value)
    .sort()
    .reduce<Record<string, unknown>>((acc, key) => {
      acc[key] = sortForStableStringify(value[key])
      return acc
    }, {})
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
