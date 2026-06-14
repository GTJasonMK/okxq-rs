import type { NumericValue } from './types'

export function formatMoneyValue(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null) return '--'
  if (Math.abs(finite) >= 1000) return finite.toLocaleString(undefined, { maximumFractionDigits: 2 })
  return finite.toLocaleString(undefined, { maximumFractionDigits: 4 })
}

export function formatSignedMoney(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null) return '--'
  return `${finite >= 0 ? '+' : ''}${formatMoneyValue(finite)}`
}

export function formatSignedPercent(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null) return '--'
  return `${finite >= 0 ? '+' : ''}${finite.toFixed(2)}%`
}

export function formatQuantity(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null) return '--'
  if (Math.abs(finite) >= 1000) return finite.toLocaleString(undefined, { maximumFractionDigits: 2 })
  return finite.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

export function formatLeverage(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null || finite <= 0) return '--'
  return `${finite.toFixed(finite % 1 === 0 ? 0 : 1)}x`
}

export function formatEventTime(timestamp: number) {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--'
  return new Date(timestamp).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

export function formatShortDate(timestamp: number) {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--'
  return new Date(timestamp).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })
}

export function positionClass(side: string) {
  if (side === 'long' || side === 'portfolio') return 'positive'
  if (side === 'short') return 'negative'
  return ''
}

export function pnlClass(value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite === null || finite === 0) return ''
  return finite > 0 ? 'positive' : 'negative'
}

export function compactSymbol(symbol?: string) {
  return symbol?.split('-')[0] || '--'
}

export function formatPercentValue(value: NumericValue, decimals: number) {
  const finite = finiteNumber(value)
  if (finite === null) return '--'
  return `${finite.toFixed(decimals)}%`
}

export function displayPositionNumber(side: string, value: NumericValue) {
  const finite = finiteNumber(value)
  if (finite !== null) return finite
  return isFlatPositionSide(side) ? 0 : null
}

export function isFlatPositionSide(side: string) {
  return side === 'flat' || side === ''
}

export function firstFiniteNumber(...values: NumericValue[]) {
  for (const value of values) {
    const finite = finiteNumber(value)
    if (finite !== null) return finite
  }
  return null
}

export function absNumber(value: NumericValue) {
  const finite = finiteNumber(value)
  return finite === null ? null : Math.abs(finite)
}

export function finiteNumber(value: NumericValue) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}
