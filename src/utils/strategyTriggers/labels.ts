export function entryLabel(side: string, posSide = '') {
  const positionSide = inferredPositionSide('open', side, posSide)
  if (positionSide === 'short') return '开空'
  if (positionSide === 'long') return '开多'
  return actionSideLabel(side)
}

export function exitLabel(side = '', posSide = '', pnl?: number) {
  const positionSide = inferredPositionSide('close', side, posSide)
  const prefix = positionSide === 'short' ? '平空' : positionSide === 'long' ? '平多' : '平仓'
  if (pnl === undefined || !Number.isFinite(pnl)) return prefix
  return `${prefix}${pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}`
}

export function actionSideLabel(side: string) {
  if (side === 'buy') return '买'
  if (side === 'sell') return '卖'
  return '动作'
}

function inferredPositionSide(action: 'open' | 'close', side: string, posSide = '') {
  if (posSide === 'short' || posSide === 'long') return posSide
  if (action === 'close') return side === 'buy' ? 'short' : side === 'sell' ? 'long' : ''
  return side === 'sell' ? 'short' : side === 'buy' ? 'long' : ''
}
