export function formatPlainNumber(value: number): string {
  if (!Number.isFinite(value)) return '--'
  return Math.trunc(value).toLocaleString('en-US')
}

export function formatCount(value: number): string {
  return Math.trunc(value).toLocaleString('en-US')
}

function numericValue(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function formatRate(value: unknown): string {
  const number = numericValue(value)
  if (number === null) return '--'
  return `${number.toFixed(6)} (${(number * 100).toFixed(4)}%)`
}

export function formatRatio(value: unknown): string {
  const number = numericValue(value)
  if (number === null) return '--'
  return `${number.toFixed(4)} (${(number * 100).toFixed(2)}%)`
}

export function formatLeverage(value: unknown): string {
  const number = numericValue(value)
  if (number === null || number <= 0) return '--'
  return `${number.toFixed(number % 1 === 0 ? 0 : 1)}x`
}

export function formatBars(value: unknown): string {
  const number = numericValue(value)
  if (number === null) return '--'
  return `${Math.trunc(number)} 根`
}

export function formatSignedPlainNumber(value: unknown): string {
  const number = numericValue(value)
  if (number === null) return '--'
  return `${number >= 0 ? '+' : ''}${number.toLocaleString('en-US', { maximumFractionDigits: 6 })}`
}
