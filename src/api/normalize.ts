type AnyRecord = Record<string, unknown>

export function isRecord(value: unknown): value is AnyRecord {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

export function recordFrom(value: unknown): AnyRecord {
  return isRecord(value) ? value : {}
}

export function arrayValue<T = unknown>(value: unknown): T[] {
  return Array.isArray(value) ? value as T[] : []
}

export function arrayRecords(value: unknown): AnyRecord[] {
  return arrayValue<unknown>(value).filter(isRecord)
}

export function stringValue(value: unknown, defaultValue = ''): string {
  return typeof value === 'string' ? value : defaultValue
}

export function numberValue(value: unknown, defaultValue = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : defaultValue
}

export function nullableNumberValue(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function booleanValue(value: unknown, defaultValue = false): boolean {
  return typeof value === 'boolean' ? value : defaultValue
}

export function timestampNumber(value: unknown, defaultValue = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : defaultValue
}

export function nullableTimestampNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function timestampString(value: unknown, defaultValue = 0): number {
  if (typeof value !== 'string') return defaultValue
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : defaultValue
}
