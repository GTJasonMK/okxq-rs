import { isRecord } from '@/api/normalize'

export function cloneParams(params: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(params)) as Record<string, unknown>
}

export function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map(key =>
      `${JSON.stringify(key)}:${stableJson(value[key])}`
    ).join(',')}}`
  }
  return JSON.stringify(value) ?? 'null'
}
