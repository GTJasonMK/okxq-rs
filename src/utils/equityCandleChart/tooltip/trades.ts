import type { BacktestTrade } from '@/types'
import type { HoverTooltipEvent } from '@/utils/equityCandleChart/types'
import {
  compactSymbol,
  formatEventTime,
  formatSignedMoney,
  pnlClass,
} from '@/utils/equityCandleChart/format'

export function tooltipEvent(trade: BacktestTrade, index: number): HoverTooltipEvent {
  const pnl = trade.action === 'funding'
    ? trade.funding
    : trade.action === 'close'
      ? trade.pnl
      : Number.NaN
  return {
    key: `${trade.timestamp}-${trade.action}-${trade.side}-${index}`,
    time: formatEventTime(trade.timestamp),
    symbol: compactSymbol(trade.symbol),
    label: tradeLabel(trade),
    sideClass: tradeSideClass(trade),
    pnl: Number.isFinite(pnl) && pnl !== 0 ? formatSignedMoney(pnl) : '--',
    pnlClass: pnlClass(pnl),
  }
}

function tradeLabel(trade: BacktestTrade) {
  if (trade.action === 'funding') return '资金费'
  const action = trade.action === 'close' ? '平' : '开'
  return `${action}${tradePositionLabel(trade)}`
}

function tradeSideClass(trade: BacktestTrade) {
  if (trade.action === 'funding') return trade.funding >= 0 ? 'positive' : 'negative'
  if (trade.action === 'close') return trade.pnl >= 0 ? 'positive' : 'negative'
  return tradePositionSide(trade) === 'short' ? 'negative' : 'positive'
}

function tradePositionLabel(trade: BacktestTrade) {
  return tradePositionSide(trade) === 'short' ? '空' : '多'
}

function tradePositionSide(trade: BacktestTrade) {
  if (trade.pos_side === 'short' || trade.pos_side === 'long') return trade.pos_side
  if (trade.action === 'close') return trade.side === 'buy' ? 'short' : 'long'
  return trade.side === 'sell' ? 'short' : 'long'
}
