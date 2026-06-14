export function numberValue(value: unknown, defaultValue = 0): number {
  const parsed = finiteNumber(value)
  return parsed === null ? defaultValue : parsed
}

export function nullableNumberValue(value: unknown): number | null {
  return finiteNumber(value)
}

export function timestampNumber(value: unknown, defaultValue = 0): number {
  const parsed = finiteNumber(value)
  return parsed === null ? defaultValue : parsed
}

export function nullableTimestampNumber(value: unknown): number | null {
  return finiteNumber(value)
}

function finiteNumber(value: unknown): number | null {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : null
  }
  if (typeof value !== 'string') return null
  const text = value.trim()
  if (!isDecimalNumberText(text)) return null
  const parsed = Number(text)
  return Number.isFinite(parsed) ? parsed : null
}

function isDecimalNumberText(value: string): boolean {
  return /^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(value)
}
