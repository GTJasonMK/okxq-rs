import { LATEST_CANDLE_VIEW_RATIO } from '@/utils/marketView/constants'

export function chartRightPaddingForLatestAnchor(
  visibleDataBars: number,
  ratio = LATEST_CANDLE_VIEW_RATIO,
) {
  const normalizedBars = Math.max(1, Math.floor(Number.isFinite(visibleDataBars) ? visibleDataBars : 1))
  const normalizedRatio = Math.min(Math.max(ratio, 0.05), 0.95)
  return Math.max(0, Math.ceil(normalizedBars * (1 - normalizedRatio) / normalizedRatio))
}

export function latestAnchoredVisibleLogicalRange(
  dataLength: number,
  visibleDataBars: number,
) {
  const normalizedLength = Math.max(0, Math.floor(Number.isFinite(dataLength) ? dataLength : 0))
  if (normalizedLength <= 1) return null
  const normalizedVisibleBars = Math.min(
    normalizedLength,
    Math.max(1, Math.floor(Number.isFinite(visibleDataBars) ? visibleDataBars : normalizedLength)),
  )
  const rightPadding = chartRightPaddingForLatestAnchor(normalizedVisibleBars)
  return {
    from: Math.max(0, normalizedLength - normalizedVisibleBars),
    to: normalizedLength - 1 + rightPadding,
    rightPadding,
  }
}
