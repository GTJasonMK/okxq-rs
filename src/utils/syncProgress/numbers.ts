function finiteNumber(value: unknown, defaultValue = 0) {
  return typeof value === 'number' && Number.isFinite(value) ? value : defaultValue
}

export function nonNegative(value: unknown) {
  return Math.max(0, Math.round(finiteNumber(value)))
}

export function clampProgress(value: unknown) {
  return clampPercent(finiteNumber(value))
}

export function clampPercent(value: number) {
  return Math.max(0, Math.min(100, finiteNumber(value)))
}

export function bounded(value: number, limit: number) {
  return Math.max(0, Math.min(finiteNumber(value), Math.max(0, finiteNumber(limit))))
}

export function scaleProgress(value: number, start: number, end: number) {
  const safeValue = finiteNumber(value)
  const safeStart = finiteNumber(start)
  const safeEnd = finiteNumber(end)
  if (safeEnd <= safeStart) return clampPercent(safeValue)
  return clampPercent(Math.round(((safeValue - safeStart) * 100) / (safeEnd - safeStart)))
}
