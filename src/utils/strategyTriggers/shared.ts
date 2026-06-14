import type { StrategyTriggerMarker } from '@/types/strategy-visualization'

export function firstFinite(...values: Array<number | null | undefined>) {
  return values.find(validPositive) ?? 0
}

export function isValidTriggerMarker(marker: StrategyTriggerMarker) {
  return validPositive(marker.timestamp) && validPositive(marker.price)
}

export function validPositive(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

export function compactInstId(instId?: string) {
  if (!instId) return ''
  return instId.split('-')[0] || instId
}

export function sortMarkers<T extends { timestamp: number }>(markers: T[]): T[] {
  return [...markers].sort((left, right) => left.timestamp - right.timestamp)
}
