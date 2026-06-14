import type { BacktestPositionSnapshot } from '@/types'
import { snapshotPositionLabel } from '@/utils/strategyExecution'
import type {
  HoverTooltipPosition,
  NumericValue,
} from '@/utils/equityCandleChart/types'
import {
  absNumber,
  finiteNumber,
  firstFiniteNumber,
  formatMoneyValue,
  formatQuantity,
  formatSignedMoney,
  formatSignedPercent,
  pnlClass,
  positionClass,
} from '@/utils/equityCandleChart/format'

export function tooltipPositions(
  positions: BacktestPositionSnapshot[],
  limit = 8,
): HoverTooltipPosition[] {
  return positions.slice(0, Math.max(0, limit)).map((position, index) => {
    const pnl = finiteNumber(position.unrealized_pnl)
    const entryNotional = absNumber(position.entry_notional)
    const returnPct = entryNotional !== null && entryNotional > 0 && pnl !== null
      ? pnl / entryNotional * 100
      : finiteNumber(position.unrealized_pnl_pct)
    const notional = firstFiniteNumber(position.position_notional, position.notional, position.entry_notional)
    return {
      key: `${position.symbol}-${position.side}-${position.entry_timestamp}-${index}`,
      symbol: position.symbol || '--',
      side: positionSideLabel(position.side),
      sideClass: positionClass(position.side),
      quantity: formatQuantity(position.quantity),
      entryPrice: formatMoneyValue(position.entry_price),
      markPrice: formatMoneyValue(position.mark_price),
      notional: formatMoneyValue(absNumber(notional)),
      pnl: formatSignedMoney(pnl),
      pnlClass: pnlClass(pnl),
      returnPct: formatSignedPercent(returnPct),
      returnClass: pnlClass(returnPct),
    }
  })
}

export function positionDetailLabel(side: string, notional: NumericValue) {
  const position = snapshotPositionLabel(side)
  if (position === '空仓') return position
  return `${position} · ${formatMoneyValue(absNumber(notional))}`
}

export function emptyPositionLabel(side: string) {
  return side === 'flat' || side === ''
    ? 'K线结束时空仓'
    : '当前结果只有聚合持仓，需重新回测生成逐仓位快照'
}

function positionSideLabel(side: string) {
  if (side === 'long') return '多单'
  if (side === 'short') return '空单'
  return snapshotPositionLabel(side)
}
