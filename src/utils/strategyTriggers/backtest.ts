import type { BacktestTrade } from '@/types'
import type { StrategyTriggerKind, StrategyTriggerMarker } from '@/types/strategy-visualization'
import {
  compactInstId,
  firstFinite,
  sortMarkers,
  validPositive,
} from '@/utils/strategyTriggers/shared'
import {
  entryLabel,
  exitLabel,
} from '@/utils/strategyTriggers/labels'

export function backtestTradesToMarkers(trades: BacktestTrade[]): StrategyTriggerMarker[] {
  const markers: StrategyTriggerMarker[] = []
  trades.forEach((trade, index) => {
    const marker = backtestEventToMarker(trade, index)
    if (marker) markers.push(marker)
  })
  return sortMarkers(markers)
}

function backtestEventToMarker(trade: BacktestTrade, index: number): StrategyTriggerMarker | null {
  if (trade.action !== 'open' && trade.action !== 'close') return null
  const timestamp = trade.timestamp
  const price = firstFinite(trade.price, trade.entry_price, trade.exit_price)
  if (!validPositive(timestamp) || !validPositive(price)) return null
  const kind: StrategyTriggerKind = trade.action === 'close' ? 'exit' : 'entry'
  const eventLabel = kind === 'entry'
    ? entryLabel(trade.side, trade.pos_side)
    : exitLabel(trade.side, trade.pos_side, trade.pnl)
  const symbol = compactInstId(trade.symbol)
  return {
    id: `bt-event-${index}-${timestamp}`,
    timestamp,
    price,
    side: trade.side,
    kind,
    source: 'backtest',
    label: symbol ? `${symbol} ${eventLabel}` : eventLabel,
    instId: trade.symbol,
    reason: trade.reason,
    detail: trade.reason,
    pnl: trade.action === 'close' ? trade.pnl : undefined,
    pnlPct: trade.pnl_pct,
  }
}
