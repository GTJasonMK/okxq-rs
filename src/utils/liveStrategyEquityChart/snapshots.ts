import type {
  BacktestEquitySnapshot,
  BacktestPositionSnapshot,
  LiveEquityHistory,
  LiveEquitySnapshot,
} from '@/types'
import {
  compareEquitySnapshotsByTime,
  equitySnapshotTimestamp,
  isValidEquitySnapshot,
} from '@/utils/liveStrategyCore/equity'
import {
  finiteNumber,
  positiveNumber,
} from '@/utils/liveStrategyEquityChart/numbers'

export function liveEquitySnapshotsForChart(
  history: LiveEquityHistory | null,
): BacktestEquitySnapshot[] {
  if (!history?.snapshots?.length) return []

  return [...history.snapshots]
    .filter(isValidEquitySnapshot)
    .sort(compareEquitySnapshotsByTime)
    .map(liveSnapshotForChart)
}

function liveSnapshotForChart(snapshot: LiveEquitySnapshot): BacktestEquitySnapshot {
  const time = equitySnapshotTimestamp(snapshot)
  const equity = finiteNumber(snapshot.equity) ?? 0
  const price = positiveNumber(snapshot.price)
  const quantity = positiveNumber(snapshot.quantity)
  const notional = price !== null && quantity !== null ? price * quantity : null
  const positionSide = normalizedPositionSide(snapshot.position_side, quantity, notional)
  const flat = isFlatPosition(positionSide)
  const positionNotional = flat ? 0 : notional
  const leverage = equity > 0 && positionNotional !== null ? Math.abs(positionNotional) / equity : 1
  const positions = livePositionForChart(snapshot, positionSide, notional)

  return {
    time,
    equity,
    cash: null,
    position_value: positionNotional,
    position_notional: positionNotional,
    unrealized_pnl: finiteNumber(snapshot.unrealized_pnl),
    position_side: positionSide,
    leverage,
    positions,
  }
}

function livePositionForChart(
  snapshot: LiveEquitySnapshot,
  positionSide: string,
  notional: number | null,
): BacktestPositionSnapshot[] {
  if (isFlatPosition(positionSide)) return []

  const quantity = positiveNumber(snapshot.quantity)
  const entryPrice = positiveNumber(snapshot.entry_price)
  const markPrice = positiveNumber(snapshot.price)
  const entryNotional = entryPrice !== null && quantity !== null ? entryPrice * quantity : null
  const unrealizedPnl = finiteNumber(snapshot.unrealized_pnl)
  const hasPositionDetail = quantity !== null
    || entryPrice !== null
    || markPrice !== null
    || notional !== null
    || (unrealizedPnl !== null && unrealizedPnl !== 0)

  if (!hasPositionDetail) return []

  return [{
    symbol: snapshot.inst_id || snapshot.symbol || '--',
    side: positionSide,
    inst_type: snapshot.inst_type || '',
    timeframe: snapshot.timeframe || '',
    entry_price: entryPrice,
    quantity,
    entry_timestamp: null,
    entry_notional: entryNotional,
    entry_reason: '',
    reason: 'live_equity_snapshot',
    stop_loss: null,
    take_profit: null,
    planned_exit_time: null,
    planned_exit_reason: '',
    planned_hold_bars: null,
    mark_price: markPrice,
    notional,
    position_notional: notional,
    unrealized_pnl: unrealizedPnl,
    unrealized_pnl_pct: entryNotional !== null && entryNotional > 0 && unrealizedPnl !== null
      ? unrealizedPnl / entryNotional * 100
      : null,
  }]
}

function normalizedPositionSide(
  rawSide: string,
  quantity: number | null,
  notional: number | null,
) {
  const side = rawSide.trim().toLowerCase()
  if (side === 'long' || side === 'short' || side === 'flat') return side
  if (side === 'portfolio' || side === 'multi' || side === 'mixed' || side === 'hedged') {
    return 'portfolio'
  }
  if ((quantity !== null && quantity > 0) || (notional !== null && notional > 0)) return 'portfolio'
  return 'flat'
}

function isFlatPosition(side: string) {
  return side === '' || side === 'flat'
}
