export function okxStringValue(value: unknown, defaultValue = ''): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' && Number.isFinite(value)) return String(value)
  return defaultValue
}

export function okxNumberValue(value: unknown, defaultValue = 0): number {
  const parsed = okxNullableNumberValue(value)
  return parsed ?? defaultValue
}

export function okxNullableNumberValue(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string') {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

export function okxPositiveNumberValue(value: unknown): number | null {
  const parsed = okxNullableNumberValue(value)
  return parsed !== null && parsed > 0 ? parsed : null
}

export function okxTimestampValue(value: unknown, defaultValue = 0): number {
  return okxNumberValue(value, defaultValue)
}
