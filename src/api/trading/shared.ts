export function modeParams(mode?: string): Record<string, string> {
  return mode ? { mode } : {}
}

export function isPresent<TValue>(value: TValue | null): value is TValue {
  return value !== null
}
