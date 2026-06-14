import { normalizeBaseSymbol } from '@/api/marketNormalize'
import type {
  InstType,
  SyncJob,
  SyncRecord,
  Timeframe,
  WatchedSymbol,
  WatchedSymbolSyncPlan,
} from '@/types'
import type {
  InventoryRow,
  PlanRow,
  WatchedRow,
} from '@/types/dataCenter'
import {
  DEFAULT_UNIFIED_SYNC_DAYS,
  timeframeOrder,
} from '@/utils/syncPlans'
import {
  jobRelevantToRow,
  summarizeJobs,
} from '@/utils/dataCenter/syncJobs'
import {
  buildPlanRowsFromScopeRecords,
  buildSyncRecordScopeIndex,
  syncRecordScopeKey,
  type SyncRecordScopeIndex,
} from '@/utils/dataCenter/watchlist/plans'
import { effectivePlansForRow } from '@/utils/dataCenter/watchlist/plans'

type WatchScope = { instId: string; instType: InstType }

type WatchScopeSource = WatchScope & {
  records: SyncRecord[]
}

export type WatchedRowSource = {
  row: Omit<WatchedRow, 'jobs' | 'jobSummary'>
  scopes: WatchScopeSource[]
  effectiveTimeframes: Set<string>
  enabledPlans: WatchedSymbolSyncPlan[]
}

type WatchlistJobContext = {
  jobsByScope: Map<string, SyncJob[]>
}

type WatchedRowsCacheEntry = {
  source: WatchedRowSource
  jobsKey: string
  row: WatchedRow
}

type WatchedRowSourceCacheEntry = {
  source: WatchedRowSource
  item: WatchedSymbol | InventoryRow
  enabledPlans: WatchedSymbolSyncPlan[] | null
  scopeKeys: string[]
  recordRefs: SyncRecord[][]
}

const EMPTY_SCOPE_RECORDS: SyncRecord[] = []

export function countEnabledInstruments(watchedSymbols: WatchedSymbol[]) {
  return watchedSymbols.reduce((total, item) => (
    total + (item.sync_spot ? 1 : 0) + (item.sync_swap ? 1 : 0)
  ), 0)
}

export function managedPlanLabelText(plans: WatchedSymbolSyncPlan[]) {
  return plans.map(plan => plan.timeframe).join('/') || '--'
}

export function buildWatchedRows(
  watchedSymbols: WatchedSymbol[],
  syncJobs: SyncJob[],
  syncRecords: SyncRecord[] = [],
  enabledPlans: WatchedSymbolSyncPlan[] = [],
  inventoryRows: InventoryRow[] = [],
): WatchedRow[] {
  return buildWatchedRowsFromSources(
    buildWatchedRowSources(watchedSymbols, syncRecords, enabledPlans, inventoryRows),
    syncJobs,
  )
}

export function buildWatchedRowSources(
  watchedSymbols: WatchedSymbol[],
  syncRecords: SyncRecord[] = [],
  enabledPlans: WatchedSymbolSyncPlan[] = [],
  inventoryRows: InventoryRow[] = [],
  recordsByScope: SyncRecordScopeIndex = buildSyncRecordScopeIndex(syncRecords),
): WatchedRowSource[] {
  const rows = watchedSymbols.map(item => watchedRowSourceFromSymbol(item, recordsByScope, enabledPlans))
  const watchedSet = new Set(rows.map(source => source.row.symbol))
  for (const inventory of inventoryRows) {
    const symbol = normalizeBaseSymbol(inventory.symbol)
    if (!symbol || watchedSet.has(symbol)) continue
    rows.push(watchedRowSourceFromInventory(inventory, recordsByScope))
    watchedSet.add(symbol)
  }
  return rows
}

export function createWatchedRowSourcesBuilder() {
  const cache = new Map<string, WatchedRowSourceCacheEntry>()

  return (
    watchedSymbols: WatchedSymbol[],
    syncRecords: SyncRecord[] = [],
    enabledPlans: WatchedSymbolSyncPlan[] = [],
    inventoryRows: InventoryRow[] = [],
    recordsByScope: SyncRecordScopeIndex = buildSyncRecordScopeIndex(syncRecords),
  ) => {
    const rows: WatchedRowSource[] = []
    const nextCache = new Map<string, WatchedRowSourceCacheEntry>()
    const watchedSet = new Set<string>()
    for (const item of watchedSymbols) {
      const symbol = normalizeBaseSymbol(item.symbol)
      if (!symbol) continue
      const entry = watchedRowSourceFromSymbolWithCache(
        item,
        recordsByScope,
        enabledPlans,
        cache.get(symbol),
      )
      rows.push(entry.source)
      nextCache.set(symbol, entry)
      watchedSet.add(symbol)
    }
    for (const inventory of inventoryRows) {
      const symbol = normalizeBaseSymbol(inventory.symbol)
      if (!symbol || watchedSet.has(symbol)) continue
      const entry = watchedRowSourceFromInventoryWithCache(
        inventory,
        recordsByScope,
        cache.get(symbol),
      )
      rows.push(entry.source)
      nextCache.set(symbol, entry)
      watchedSet.add(symbol)
    }

    cache.clear()
    for (const [symbol, entry] of nextCache) cache.set(symbol, entry)
    return rows
  }
}

export function buildWatchedRowsFromSources(
  sources: WatchedRowSource[],
  syncJobs: SyncJob[],
): WatchedRow[] {
  const context = buildWatchlistJobContext(syncJobs)
  return sources.map(source => buildWatchedRowFromSource(source, context))
}

export function createWatchedRowsBuilder() {
  const cache = new Map<string, WatchedRowsCacheEntry>()

  return (sources: WatchedRowSource[], syncJobs: SyncJob[]) => {
    const context = buildWatchlistJobContext(syncJobs)
    const nextCache = new Map<string, WatchedRowsCacheEntry>()
    const rows = sources.map((source) => {
      const jobs = relevantJobsForSource(context, source)
      const jobsKey = watchedRowJobsKey(jobs)
      const cached = cache.get(source.row.symbol)
      if (cached && cached.source === source && cached.jobsKey === jobsKey) {
        nextCache.set(source.row.symbol, cached)
        return cached.row
      }
      const row = watchedRowWithJobs(source, jobs)
      nextCache.set(source.row.symbol, { source, jobsKey, row })
      return row
    })

    cache.clear()
    for (const [symbol, entry] of nextCache) cache.set(symbol, entry)
    return rows
  }
}

function watchedRowSourceFromSymbol(
  item: WatchedSymbol,
  recordsByScope: SyncRecordScopeIndex,
  enabledPlans: WatchedSymbolSyncPlan[],
): WatchedRowSource {
  const enabledScopes = watchedSymbolScopes(item)
  const effectiveTimeframes = new Set(effectivePlansForRow(item, enabledPlans).map(plan => plan.timeframe))
  const scopes = sourceScopes(recordsByScope, enabledScopes)
  for (const record of recordsForSourceScopes(scopes)) {
    effectiveTimeframes.add(record.timeframe)
  }
  return { row: { ...item }, scopes, effectiveTimeframes, enabledPlans }
}

function watchedRowSourceFromSymbolWithCache(
  item: WatchedSymbol,
  recordsByScope: SyncRecordScopeIndex,
  enabledPlans: WatchedSymbolSyncPlan[],
  cached: WatchedRowSourceCacheEntry | undefined,
): WatchedRowSourceCacheEntry {
  const scopes = sourceScopes(recordsByScope, watchedSymbolScopes(item))
  if (sourceCacheMatches(cached, item, enabledPlans, scopes)) {
    return cached
  }
  return {
    source: watchedRowSourceFromSymbol(item, recordsByScope, enabledPlans),
    item,
    enabledPlans,
    ...sourceCacheKeys(scopes),
  }
}

function watchedRowSourceFromInventory(
  item: InventoryRow,
  recordsByScope: SyncRecordScopeIndex,
): WatchedRowSource {
  const symbol = normalizeBaseSymbol(item.symbol)
  const spotMarket = item.markets.SPOT
  const swapMarket = item.markets.SWAP
  const enabledScopes = inventoryScopes(item)
  const recordTimeframes = new Set<Timeframe>()
  const scopes = sourceScopes(recordsByScope, enabledScopes)
  for (const record of recordsForSourceScopes(scopes)) {
    recordTimeframes.add(record.timeframe)
  }
  const inventoryTimeframes = Array.from(recordTimeframes).sort((left, right) => timeframeOrder(left) - timeframeOrder(right))
  const row: WatchedSymbol = {
    symbol,
    base_ccy: item.base_ccy || symbol.split('-')[0] || symbol,
    spot_inst_id: spotMarket?.inst_id || symbol,
    swap_inst_id: swapMarket?.inst_id || `${symbol}-SWAP`,
    sync_spot: Boolean(spotMarket),
    sync_swap: Boolean(swapMarket),
    archive_all_history: false,
    sync_days: DEFAULT_UNIFIED_SYNC_DAYS,
    sync_plans: inventoryTimeframes.map(timeframe => ({
      timeframe,
      enabled: true,
      bootstrap_days: DEFAULT_UNIFIED_SYNC_DAYS,
      archive_mode: 'rolling',
    })),
    created_at: '',
    updated_at: '',
  }
  const effectiveTimeframes = new Set(inventoryTimeframes)
  return {
    row: {
      ...row,
      inventory_only: true,
      inventory_timeframes: inventoryTimeframes,
    },
    scopes,
    effectiveTimeframes,
    enabledPlans: [],
  }
}

function watchedRowSourceFromInventoryWithCache(
  item: InventoryRow,
  recordsByScope: SyncRecordScopeIndex,
  cached: WatchedRowSourceCacheEntry | undefined,
): WatchedRowSourceCacheEntry {
  const scopes = sourceScopes(recordsByScope, inventoryScopes(item))
  if (sourceCacheMatches(cached, item, null, scopes)) {
    return cached
  }
  return {
    source: watchedRowSourceFromInventory(item, recordsByScope),
    item,
    enabledPlans: null,
    ...sourceCacheKeys(scopes),
  }
}

function buildWatchlistJobContext(syncJobs: SyncJob[]): WatchlistJobContext {
  const jobsByScope = new Map<string, SyncJob[]>()
  for (const job of syncJobs) {
    pushGrouped(jobsByScope, scopeKey(job.inst_id, job.inst_type), job)
  }
  return { jobsByScope }
}

function sourceScopes(recordsByScope: SyncRecordScopeIndex, scopes: WatchScope[]) {
  const sourceScopes: WatchScopeSource[] = []
  const seen = new Set<string>()
  for (const scope of scopes) {
    const key = scopeKey(scope.instId, scope.instType)
    if (seen.has(key)) continue
    seen.add(key)
    sourceScopes.push({ ...scope, records: recordsByScope.get(key) ?? EMPTY_SCOPE_RECORDS })
  }
  return sourceScopes
}

function recordsForSourceScopes(scopes: WatchScopeSource[]) {
  const records: SyncRecord[] = []
  for (const scope of scopes) records.push(...scope.records)
  return records
}

function buildWatchedRowFromSource(source: WatchedRowSource, context: WatchlistJobContext) {
  return watchedRowWithJobs(source, relevantJobsForSource(context, source))
}

function relevantJobsForSource(context: WatchlistJobContext, source: WatchedRowSource) {
  const jobs: SyncJob[] = []
  const seenScopes = new Set<string>()
  const seenTasks = new Set<string>()
  for (const scope of source.scopes) {
    const key = scopeKey(scope.instId, scope.instType)
    if (seenScopes.has(key)) continue
    seenScopes.add(key)
    for (const job of context.jobsByScope.get(key) ?? []) {
      if (seenTasks.has(job.task_id)) continue
      if (jobRelevantToRow(job, [scope], source.effectiveTimeframes, scope.records)) {
        jobs.push(job)
        seenTasks.add(job.task_id)
      }
    }
  }
  return jobs
}

function watchedRowWithJobs(source: WatchedRowSource, jobs: SyncJob[]) {
  const row: WatchedRow = { ...source.row, jobs, jobSummary: summarizeJobs(jobs) }
  row.planRowsByInstType = planRowsByInstType(row, source)
  return row
}

function planRowsByInstType(row: WatchedRow, source: WatchedRowSource) {
  const rows: Partial<Record<InstType, PlanRow[]>> = {}
  for (const scope of source.scopes) {
    rows[scope.instType] = buildPlanRowsFromScopeRecords(
      row,
      scope.instType,
      scope.records,
      source.enabledPlans,
    )
  }
  return rows
}

function watchedRowJobsKey(jobs: SyncJob[]) {
  return jobs.map(job => [
    job.task_id,
    job.status,
    job.progress,
    job.message ?? '',
    job.error ?? '',
    job.fetched_count ?? '',
    job.target_fetch_count ?? '',
    job.saved_count ?? '',
    job.target_save_count ?? '',
    job.derived_count ?? '',
    job.target_derive_count ?? '',
    job.batches ?? '',
    job.target_batches ?? '',
    job.api_calls ?? '',
    job.candle_count ?? '',
    job.history_complete ?? '',
    job.updated_at ?? '',
    job.finished_at ?? '',
  ].join(':')).join('|')
}

function pushGrouped<T>(groups: Map<string, T[]>, key: string, item: T) {
  const items = groups.get(key)
  if (items) {
    items.push(item)
  } else {
    groups.set(key, [item])
  }
}

function scopeKey(instId: string, instType: InstType) {
  return syncRecordScopeKey(instId, instType)
}

function watchedSymbolScopes(item: WatchedSymbol): WatchScope[] {
  return [
    item.sync_spot ? { instId: item.spot_inst_id, instType: 'SPOT' } : null,
    item.sync_swap ? { instId: item.swap_inst_id, instType: 'SWAP' } : null,
  ].filter((scope): scope is WatchScope => !!scope)
}

function inventoryScopes(item: InventoryRow): WatchScope[] {
  const scopes: WatchScope[] = []
  const spotMarket = item.markets.SPOT
  const swapMarket = item.markets.SWAP
  if (spotMarket) scopes.push({ instId: spotMarket.inst_id, instType: 'SPOT' })
  if (swapMarket) scopes.push({ instId: swapMarket.inst_id, instType: 'SWAP' })
  return scopes
}

function sourceCacheMatches(
  cached: WatchedRowSourceCacheEntry | undefined,
  item: WatchedSymbol | InventoryRow,
  enabledPlans: WatchedSymbolSyncPlan[] | null,
  scopes: WatchScopeSource[],
): cached is WatchedRowSourceCacheEntry {
  if (!cached || cached.item !== item || cached.enabledPlans !== enabledPlans) return false
  if (cached.scopeKeys.length !== scopes.length) return false
  for (let index = 0; index < scopes.length; index += 1) {
    if (cached.scopeKeys[index] !== scopeKey(scopes[index].instId, scopes[index].instType)) return false
    if (cached.recordRefs[index] !== scopes[index].records) return false
  }
  return true
}

function sourceCacheKeys(scopes: WatchScopeSource[]) {
  return {
    scopeKeys: scopes.map(scope => scopeKey(scope.instId, scope.instType)),
    recordRefs: scopes.map(scope => scope.records),
  }
}
