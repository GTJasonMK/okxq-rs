import { apiDelete, apiGet, apiPost } from '../client'
import { arrayRecords, stringValue } from '../normalize'
import type {
  WatchedSymbol,
  WatchedSymbolSyncPlan,
  SyncJob,
} from '@/types/market'
import {
  inferInstTypeFromId,
  normalizeBaseSymbol,
  normalizeInstId,
  normalizeWatchedSymbol,
  normalizeWatchMutationResult,
} from '../marketNormalize'
import {
  normalizeRepairWatchedSymbolResult,
  type RepairWatchedSymbolResult,
} from './normalize'
import type { EnabledWatchInstType, EnabledWatchScope } from './types'

export function fetchWatchedSymbols() {
  return apiGet<unknown>('/api/market/watched-symbols')
    .then(data => arrayRecords(data).map(normalizeWatchedSymbol))
}

export function enabledWatchScopesFromSymbols(items: WatchedSymbol[]): EnabledWatchScope[] {
  const scopes: EnabledWatchScope[] = []
  for (const item of items) {
    if (item.sync_swap) {
      scopes.push({
        symbol: item.symbol,
        inst_id: normalizeInstId(item.swap_inst_id || item.symbol, 'SWAP'),
        inst_type: 'SWAP',
        base_ccy: item.base_ccy,
      })
    }
    if (item.sync_spot) {
      scopes.push({
        symbol: item.symbol,
        inst_id: normalizeInstId(item.spot_inst_id || item.symbol, 'SPOT'),
        inst_type: 'SPOT',
        base_ccy: item.base_ccy,
      })
    }
  }
  const seen = new Set<string>()
  return scopes.filter((scope) => {
    const key = `${scope.inst_id}:${scope.inst_type}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

function chooseEnabledWatchScope(
  watched: WatchedSymbol[],
  current?: { symbol?: string; inst_id?: string; inst_type?: string },
): EnabledWatchScope | null {
  const scopes = enabledWatchScopesFromSymbols(watched)
  if (scopes.length === 0) return null

  const currentType = stringValue(current?.inst_type).trim().toUpperCase() as EnabledWatchInstType
  const currentId = stringValue(current?.inst_id || current?.symbol).trim()
  if (currentId) {
    const inferredType = currentType === 'SWAP' || currentType === 'SPOT' ? currentType : inferInstTypeFromId(currentId)
    const normalizedId = normalizeInstId(currentId, inferredType)
    const match = scopes.find(scope => scope.inst_id === normalizedId && scope.inst_type === inferredType)
    if (match) return match
  }

  const currentSymbol = normalizeBaseSymbol(stringValue(current?.symbol))
  if (currentSymbol) {
    const match = scopes.find(scope => scope.symbol === currentSymbol)
    if (match) return match
  }

  return scopes[0]
}

export async function fetchDefaultWatchScope(current?: { symbol?: string; inst_id?: string; inst_type?: string }) {
  return chooseEnabledWatchScope(await fetchWatchedSymbols(), current)
}

export function addWatchedSymbol(symbol: string, opts?: {
  sync_spot?: boolean
  sync_swap?: boolean
  archive_all_history?: boolean
  sync_days?: number
  sync_plans?: WatchedSymbolSyncPlan[]
  auto_sync?: boolean
}) {
  return apiPost<{
    watched_symbol?: WatchedSymbol
    existed?: boolean
    sync_jobs?: SyncJob[]
    cancelled_disabled_jobs?: SyncJob[]
    started_count?: number
    reused_count?: number
    exact_gap_jobs?: number
    rule_jobs?: number
  }>('/api/market/watched-symbols', { symbol, ...opts }).then(normalizeWatchMutationResult)
}

export function deleteWatchedSymbol(symbol: string) {
  return apiDelete(`/api/market/watched-symbols/${encodeURIComponent(symbol)}`)
}

export function repairWatchedSymbol(
  symbol: string,
  opts?: { sync_spot?: boolean; sync_swap?: boolean },
): Promise<RepairWatchedSymbolResult> {
  return apiPost<unknown>(`/api/market/watched-symbols/${encodeURIComponent(symbol)}/repair`, opts)
    .then(normalizeRepairWatchedSymbolResult)
}
