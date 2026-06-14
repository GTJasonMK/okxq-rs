export function formatPrice(value: number | null | undefined, decimals = 2): string {
  if (!isFiniteNumber(value)) return '--'
  if (value >= 1) return value.toFixed(decimals)
  if (value >= 0.01) return value.toFixed(decimals + 2)
  return value.toPrecision(6)
}

export function formatVolume(value: number | null | undefined, decimals = 2): string {
  if (!isFiniteNumber(value)) return '--'
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(decimals)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(decimals)}K`
  return value.toFixed(decimals)
}

export function formatPercent(value: number | null | undefined, decimals = 2): string {
  if (!isFiniteNumber(value)) return '--'
  return `${(value * 100).toFixed(decimals)}%`
}

export function formatMoney(value: number | null | undefined, decimals = 2): string {
  if (!isFiniteNumber(value)) return '--'
  const prefix = value < 0 ? '-' : ''
  const abs = Math.abs(value)
  if (abs >= 1_000_000) return `${prefix}${(abs / 1_000_000).toFixed(decimals)}M`
  if (abs >= 1_000) return `${prefix}${(abs / 1_000).toFixed(decimals)}K`
  return `${prefix}${abs.toFixed(decimals)}`
}

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}
