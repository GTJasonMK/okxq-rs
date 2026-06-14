import { nonNegative } from './numbers'

const integerFormatter = new Intl.NumberFormat('zh-CN')

export function formatInteger(value: number) {
  return integerFormatter.format(nonNegative(value))
}

export function formatWorkProgress(done: number, total: number) {
  const safeTotal = nonNegative(total)
  const doneText = formatInteger(done)
  if (safeTotal > 0) return `${doneText} / ${formatInteger(safeTotal)}`
  return doneText
}
