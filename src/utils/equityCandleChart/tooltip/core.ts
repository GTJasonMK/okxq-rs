import type {
  BacktestEquitySnapshot,
  BacktestPositionSnapshot,
  BacktestTrade,
  Timeframe,
} from '@/types'
import { formatChartCandleTime } from '@/utils/chartTime'
import {
  selectedPositionSnapshotForEquityCandle,
  selectedSnapshotForEquityCandle,
  snapshotPositionLabel,
  tradeEventsForEquityCandle,
  type EquityCandle,
} from '@/utils/strategyExecution'
import type { HoverTooltip } from '@/utils/equityCandleChart/types'
import {
  absNumber,
  displayPositionNumber,
  firstFiniteNumber,
  formatLeverage,
  formatMoneyValue,
  formatPercentValue,
  formatSignedMoney,
  pnlClass,
  positionClass,
} from '@/utils/equityCandleChart/format'
import {
  equityCandleReturnPct,
  equityExposurePct,
} from '@/utils/equityCandleChart/metrics'
import { tooltipPosition } from '@/utils/equityCandleChart/tooltip/layout'
import {
  emptyPositionLabel,
  positionDetailLabel,
  tooltipPositions,
} from '@/utils/equityCandleChart/tooltip/positions'
import { tooltipEvent } from '@/utils/equityCandleChart/tooltip/trades'

export function equityHoverTooltip(input: {
  candle: EquityCandle
  snapshots: BacktestEquitySnapshot[]
  trades: BacktestTrade[]
  timeframe: Timeframe
  pointX: number
  pointY: number
  containerWidth: number
  containerHeight: number
}): HoverTooltip {
  const snapshot = selectedSnapshotForEquityCandle(input.snapshots, input.candle.timestamp, input.timeframe)
  const positionSnapshot = selectedPositionSnapshotForEquityCandle(input.snapshots, input.candle.timestamp, input.timeframe)
  const bucketEvents = tradeEventsForEquityCandle(input.trades, input.candle.timestamp, input.timeframe, 32)
  const change = equityCandleReturnPct(input.candle)
  const positionSide = positionSnapshot?.position_side ?? 'flat'
  const notional = displayPositionNumber(
    positionSide,
    firstFiniteNumber(positionSnapshot?.position_notional, positionSnapshot?.position_value),
  )
  const unrealized = displayPositionNumber(positionSide, positionSnapshot?.unrealized_pnl)
  const exposurePct = equityExposurePct(positionSnapshot, input.candle)
  const position = snapshotPositionLabel(positionSide)
  const snapshotPositions = positionsForTooltip(positionSnapshot)
  const positions = tooltipPositions(snapshotPositions, 32)
  const positionsTotal = snapshotPositions.length
  const positionSnapshotIsClosingSnapshot = positionSnapshot === snapshot

  return {
    ...tooltipPosition(input.pointX, input.pointY, input.containerWidth, input.containerHeight),
    time: formatChartCandleTime(input.candle.timestamp, input.timeframe),
    open: formatMoneyValue(input.candle.open),
    high: formatMoneyValue(input.candle.high),
    low: formatMoneyValue(input.candle.low),
    close: formatMoneyValue(input.candle.close),
    change: `${change >= 0 ? '+' : ''}${change.toFixed(2)}%`,
    positive: input.candle.close >= input.candle.open,
    equity: formatMoneyValue(snapshot?.equity ?? input.candle.close),
    cash: formatMoneyValue(snapshot?.cash ?? snapshot?.equity ?? input.candle.close),
    notional: formatMoneyValue(absNumber(notional)),
    unrealized: formatSignedMoney(unrealized),
    unrealizedClass: pnlClass(unrealized),
    position,
    positionDetail: positionDetailLabel(positionSide, notional),
    positionClass: positionClass(positionSide),
    exposure: formatPercentValue(exposurePct, 1),
    leverage: formatLeverage(snapshot?.leverage ?? 1),
    count: input.candle.snapshot_count,
    positionTitle: positionSnapshotIsClosingSnapshot ? '当前持仓' : 'K线内持仓',
    positionEmpty: emptyPositionLabel(positionSide),
    positions,
    positionsTotal,
    positionsMore: positionsTotal > positions.length ? `还有 ${positionsTotal - positions.length} 个持仓未显示` : '',
    eventTitle: '本K线事件',
    events: bucketEvents.map(tooltipEvent),
  }
}

function positionsForTooltip(snapshot: BacktestEquitySnapshot | null): BacktestPositionSnapshot[] {
  if (!snapshot) return []
  if (snapshot.positions && snapshot.positions.length > 0) return snapshot.positions
  const side = snapshot.position_side
  if (side === 'flat' || side === '') return []
  const notional = firstFiniteNumber(snapshot.position_notional, snapshot.position_value)
  const unrealized = snapshot.unrealized_pnl
  if (notional === null && unrealized === null) return []
  return [{
    symbol: '组合持仓',
    side,
    inst_type: '',
    timeframe: '',
    entry_price: null,
    quantity: null,
    entry_timestamp: null,
    entry_notional: null,
    entry_reason: '',
    reason: '',
    stop_loss: null,
    take_profit: null,
    planned_exit_time: null,
    planned_exit_reason: '',
    planned_hold_bars: null,
    mark_price: null,
    notional,
    position_notional: notional,
    unrealized_pnl: unrealized,
    unrealized_pnl_pct: null,
  }]
}
