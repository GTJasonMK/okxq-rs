export function pnlColor(value: number | null | undefined): string {
  if (!isFiniteNumber(value)) return 'neutral'
  if (value > 0) return 'positive'
  if (value < 0) return 'negative'
  return 'neutral'
}

export const CHART_COLORS = {
  positive: '#26a69a',
  negative: '#ef5350',
  primary: '#2962ff',
  background: '#1a1a2e',
} as const

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}
