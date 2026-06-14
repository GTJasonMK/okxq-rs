export function numberField(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

export function positiveNumberField(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : fallback
}

export function finiteNumber(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function formatCompactNumber(value: number) {
  if (!Number.isFinite(value)) return ''
  const fixed = Math.abs(value) >= 10 ? value.toFixed(1) : value.toFixed(2)
  return fixed.replace(/\.?0+$/, '')
}

export function formatPercent(value: number) {
  return `${formatCompactNumber(value * 100)}%`
}
