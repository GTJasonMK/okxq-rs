import type {
  SyncJob,
} from '@/types/market'
import {
  isRecord,
  stringValue,
} from '../../normalize'
import {
  normalizeWatchMutationResult,
} from '../../marketNormalize'

type RepairMarketSelection = { spot: boolean; swap: boolean }

export type RepairWatchedSymbolResult = {
  symbol: string
  sync_jobs: SyncJob[]
  requested_markets: RepairMarketSelection
  effective_markets: RepairMarketSelection
  started_count: number
  reused_count: number
  exact_gap_jobs: number
  rule_jobs: number
}

export function normalizeRepairWatchedSymbolResult(raw: unknown): RepairWatchedSymbolResult {
  const item = isRecord(raw) ? raw : {}
  const normalized = normalizeWatchMutationResult(item)
  return {
    symbol: stringValue(item.symbol),
    sync_jobs: normalized.sync_jobs,
    requested_markets: normalizeRepairMarketSelection(item.requested_markets),
    effective_markets: normalizeRepairMarketSelection(item.effective_markets),
    started_count: normalized.started_count ?? 0,
    reused_count: normalized.reused_count ?? 0,
    exact_gap_jobs: normalized.exact_gap_jobs ?? 0,
    rule_jobs: normalized.rule_jobs ?? 0,
  }
}

function normalizeRepairMarketSelection(raw: unknown): RepairMarketSelection {
  const item = isRecord(raw) ? raw : {}
  return {
    spot: item.spot === true,
    swap: item.swap === true,
  }
}
