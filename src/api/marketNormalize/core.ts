import type {
  SyncJob,
  WatchedSymbolSyncPlan,
} from '@/types/market'
import { stringValue as textValue } from '../normalize'

export function normalizeTimeframe(value: string): WatchedSymbolSyncPlan['timeframe'] | '' {
  const normalized = value.trim()
  if (normalized === '1m') return '1m'
  if (normalized === '3m') return '3m'
  if (normalized === '5m') return '5m'
  if (normalized === '15m') return '15m'
  if (normalized === '30m') return '30m'
  if (normalized === '1H' || normalized === '1h') return '1H'
  if (normalized === '2H' || normalized === '2h') return '2H'
  if (normalized === '4H' || normalized === '4h') return '4H'
  if (normalized === '6H' || normalized === '6h') return '6H'
  if (normalized === '12H' || normalized === '12h') return '12H'
  if (normalized === '1D' || normalized === '1d') return '1D'
  if (normalized === '1W' || normalized === '1w') return '1W'
  if (normalized === '1M') return '1M'
  return ''
}

export function timeframeOrder(value: string): number {
  return ['1m', '3m', '5m', '15m', '30m', '1H', '2H', '4H', '6H', '12H', '1D', '1W', '1M'].indexOf(value)
}

export function normalizeBaseSymbol(value: string): string {
  let normalized = value.trim().toUpperCase()
  if (!normalized) return ''
  if (normalized.endsWith('-SWAP')) normalized = normalized.slice(0, -5)
  if (!normalized.includes('-')) normalized = `${normalized}-USDT`
  return normalized
}

export function inferInstTypeFromId(value: string): 'SPOT' | 'SWAP' {
  return value.trim().toUpperCase().endsWith('-SWAP') ? 'SWAP' : 'SPOT'
}

export function normalizeInstType(value: unknown, defaultType: 'SPOT' | 'SWAP' = 'SPOT'): 'SPOT' | 'SWAP' {
  const normalized = textValue(value, defaultType).trim().toUpperCase()
  return normalized === 'SWAP' ? 'SWAP' : 'SPOT'
}

export function normalizeInstId(value: string, instType: string): string {
  let normalized = normalizeBaseSymbol(value)
  if (!normalized) return ''
  if (instType.trim().toUpperCase() === 'SWAP' && !normalized.endsWith('-SWAP')) {
    normalized = `${normalized}-SWAP`
  }
  if (instType.trim().toUpperCase() === 'SPOT' && normalized.endsWith('-SWAP')) {
    normalized = normalized.slice(0, -5)
  }
  return normalized
}

export function normalizeTimeframeList(value: unknown): Array<SyncJob['timeframe']> {
  const rawItems = Array.isArray(value) ? value : []
  const seen = new Set<string>()
  return rawItems
    .map(item => normalizeTimeframe(textValue(item)))
    .filter((timeframe): timeframe is SyncJob['timeframe'] => {
      if (!timeframe || seen.has(timeframe)) return false
      seen.add(timeframe)
      return true
    })
    .sort((a, b) => timeframeOrder(a) - timeframeOrder(b))
}
