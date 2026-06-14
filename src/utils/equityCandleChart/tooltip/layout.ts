import {
  TOOLTIP_HEIGHT,
  TOOLTIP_OFFSET,
  TOOLTIP_WIDTH,
} from '@/utils/equityCandleChart/tooltip/constants'

export function tooltipPosition(
  pointX: number,
  pointY: number,
  containerWidth: number,
  containerHeight: number,
) {
  const rightX = pointX + TOOLTIP_OFFSET
  const leftX = pointX - TOOLTIP_WIDTH - TOOLTIP_OFFSET
  const lowerY = pointY + TOOLTIP_OFFSET
  const upperY = pointY - TOOLTIP_HEIGHT - TOOLTIP_OFFSET
  return {
    x: Math.max(8, Math.min(
      rightX + TOOLTIP_WIDTH > containerWidth - 8 ? leftX : rightX,
      Math.max(8, containerWidth - TOOLTIP_WIDTH - 8),
    )),
    y: Math.max(8, Math.min(
      lowerY + TOOLTIP_HEIGHT > containerHeight - 8 ? upperY : lowerY,
      Math.max(8, containerHeight - TOOLTIP_HEIGHT - 8),
    )),
  }
}
