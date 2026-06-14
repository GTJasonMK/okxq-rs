export function snapshotPositionLabel(side: string) {
  if (side === 'long') return '多'
  if (side === 'short') return '空'
  if (side === 'portfolio') return '组合'
  return '空仓'
}
