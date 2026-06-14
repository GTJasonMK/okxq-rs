import type * as T from '@/types/live-strategy'

export function tradingMode(value: unknown): T.LiveStrategyStatus['mode'] {
  if (value === undefined || value === null || value === '') return 'simulated'
  if (value === 'live') return 'live'
  if (value === 'simulated') return 'simulated'
  throw new TypeError(`实时策略运行模式只支持 live 或 simulated，收到 ${String(value)}`)
}

export function orderSide(value: unknown, defaultValue: 'buy' | 'sell' | ''): 'buy' | 'sell' | '' {
  if (value === 'buy' || value === 'sell') return value
  return defaultValue
}

export function positionSide(value: unknown, defaultValue: string): string {
  if (
    value === 'long'
    || value === 'short'
    || value === 'flat'
    || value === 'multi'
    || value === 'portfolio'
  ) return value
  return defaultValue
}
