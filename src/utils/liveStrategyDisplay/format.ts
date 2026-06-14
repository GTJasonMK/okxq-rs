import type { LiveStrategyKpiKind } from './types'

export function pnlKind(value: number): LiveStrategyKpiKind {
  if (value > 0) return 'positive'
  if (value < 0) return 'negative'
  return 'neutral'
}

export function livePnlClass(value: number | null | undefined) {
  if (!isFiniteNumber(value)) return 'flat'
  if (value > 0) return 'positive'
  if (value < 0) return 'negative'
  return 'flat'
}

export function formatMoneyCompact(value: number) {
  if (!Number.isFinite(value) || value <= 0) return '—'
  return value.toLocaleString('zh-CN', {
    maximumFractionDigits: 2,
  })
}

export function formatSignedMoney(value: number) {
  if (!Number.isFinite(value)) return '—'
  const sign = value > 0 ? '+' : ''
  return `${sign}${value.toLocaleString('zh-CN', { maximumFractionDigits: 2 })}`
}

export function formatSignedPercentPoint(value: number) {
  if (!Number.isFinite(value)) return '—'
  const sign = value > 0 ? '+' : ''
  return `${sign}${value.toFixed(2)}%`
}

export function formatLiveDateTime(value: number | null | undefined) {
  if (!isFiniteNumber(value) || value <= 0) return '--'
  return new Date(value).toLocaleString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

export function formatLiveQuantity(value: number | null | undefined) {
  if (!isFiniteNumber(value)) return '--'
  return value >= 1 ? value.toFixed(4) : value.toFixed(6)
}

export function formatLiveAbsoluteQuantity(value: number | null | undefined) {
  if (!isFiniteNumber(value)) return '--'
  return formatLiveQuantity(Math.abs(value))
}

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}
