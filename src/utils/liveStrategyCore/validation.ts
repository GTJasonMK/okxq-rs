import {
  finiteNumber,
  formatCompactNumber,
  formatPercent,
} from '@/utils/liveStrategyCore/numbers'
import type { NumberRangeUnit } from '@/utils/liveStrategyCore/types'

export function firstNumberRangeError(items: Array<{
  label: string
  value: unknown
  min: number
  max: number
  unit: NumberRangeUnit
}>) {
  for (const item of items) {
    const value = finiteNumber(item.value)
    if (value === null) return `${item.label}必须是数字`
    if (value < item.min || value > item.max) {
      if (!Number.isFinite(item.max)) {
        return `${item.label}必须不小于 ${formatRangeValue(item.min, item.unit)}`
      }
      return `${item.label}必须在 ${formatRangeValue(item.min, item.unit)} 到 ${formatRangeValue(item.max, item.unit)} 之间`
    }
  }
  return ''
}

function formatRangeValue(value: number, unit: NumberRangeUnit) {
  if (unit === '%') return formatPercent(value)
  if (unit === '秒') return `${formatCompactNumber(value)} 秒`
  return formatCompactNumber(value)
}
