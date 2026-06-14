import type * as T from '@/types/backtest'

export function idValue(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' && Number.isInteger(value)) return value.toString()
  return ''
}

export function orderSide(value: unknown): T.BacktestTrade['side'] {
  if (value === 'buy' || value === 'sell') return value
  if (value === 'funding') return value
  return ''
}

export function positionSide(value: unknown): string {
  if (value === 'long' || value === 'short' || value === 'flat' || value === 'portfolio') return value
  return ''
}
