import type {
  InstType,
  SyncJob,
  Timeframe,
  WatchedSymbol,
} from '@/types'
import type { SyncProgressSummary } from '@/utils/syncProgress'

export type GuardianPlan = {
  timeframe: string
  enabled: boolean
  bootstrap_days: number
  archive_mode: string
}

export type GuardianSettings = {
  enabled: boolean
  scan_interval_seconds: number
  max_full_backfill_jobs_per_cycle: number
  plans: GuardianPlan[]
}

export type GuardianConfig = {
  settings: GuardianSettings
  defaults: GuardianSettings
  status?: GuardianStatus
}

export type DataCenterTab = 'watchlist' | 'collection' | 'inventory' | 'guardian'

export type DataCenterTabItem = {
  key: DataCenterTab
  label: string
  description: string
}

export type InventorySummary = {
  symbol_count: number
  managed_symbol_count: number
  managed_market_count: number
  watched_symbol_count: number
  watched_list_count: number
  watched_market_count: number
  orphan_symbol_count: number
  total_candles: number
  total_timeframe_records: number
  table_totals: Record<string, number>
}

export type InventoryMarket = {
  inst_id: string
  inst_type: InstType
  managed: boolean
  watched: boolean
  timeframe_count: number
  candle_count: number
  gap_count: number
  history_complete_count: number
  oldest_timestamp?: number | null
  newest_timestamp?: number | null
  oldest_time?: string | null
  newest_time?: string | null
  last_sync_time?: string | null
  timeframes: InventoryTimeframeRecord[]
}

export type InventoryTimeframeRecord = {
  timeframe: Timeframe
  managed: boolean
  candle_count: number
  expected_candle_count: number
  gap_count: number
  coverage_ratio: number
  history_complete: boolean
  last_sync_mode?: string | null
  last_sync_time?: string | null
  oldest_timestamp?: number | null
  newest_timestamp?: number | null
  oldest_time?: string | null
  newest_time?: string | null
}

export type ExactGapRepairPayload = {
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  start_ts: number
  end_ts: number
}

export type InventoryGapRepairPayload = ExactGapRepairPayload

export type InventoryRow = {
  symbol: string
  base_ccy?: string
  managed: boolean
  watched: boolean
  orphan: boolean
  candle_count: number
  timeframe_record_count: number
  storage_counts: Record<string, number>
  markets: Partial<Record<InstType, InventoryMarket>>
}

export type InventoryCacheRebuildResult = {
  message: string
  candle_groups_scanned: number
  sync_records_rebuilt: number
  stale_sync_records_deleted: number
  sync_records_total: number
  cached_candles_total: number
  inventory: {
    summary: InventorySummary
    rows: InventoryRow[]
  }
  progress?: InventoryCacheRebuildProgress | null
}

export type InventoryCacheRebuildProgress = {
  task_id: string
  status: 'queued' | 'running' | 'completed' | 'failed' | string
  phase: string
  progress: number
  message: string
  started_at: string
  updated_at: string
  finished_at?: string | null
  error: string
  processed_candles: number
  target_candles: number
  processed_groups: number
  target_groups: number
  scan_concurrency: number
  candle_groups_scanned: number
  sync_records_rebuilt: number
  stale_sync_records_deleted: number
  sync_records_total: number
  cached_candles_total: number
}

export type InventoryCacheRebuildStartResult = {
  reused_existing: boolean
  progress: InventoryCacheRebuildProgress | null
}

export type InventoryCacheRebuildStatus = {
  progress: InventoryCacheRebuildProgress | null
}

export type TickCollectorStatus = {
  running: boolean
  active_symbols: string[]
  book_channel: string
  total_trades_received: number
  total_bars_written: number
  last_trade_ts: number
  errors: string[]
}

export type TickCollectorActionResult = {
  message: string
  status: TickCollectorStatus
  realtime?: unknown
}

export type GuardianStatus = {
  enabled?: boolean
  active?: boolean
  policy_summary?: string
  rolling_window_timeframes?: string[]
  full_backfill_timeframes?: string[]
  watched_count?: number
  backfill_queue_size?: number
  current_inst_id?: string
  current_timeframe?: string
  current_mode?: string
  current_phase?: string
  last_successful_run_at?: string | number | null
  last_run_finished_at?: string | number | null
  backfill_queue_preview?: SyncJob[]
  last_sync_results?: SyncJob[]
  last_errors?: unknown[]
}

export type WatchedRow = WatchedSymbol & {
  inventory_only?: boolean
  inventory_timeframes?: Timeframe[]
  jobs: SyncJob[]
  jobSummary: SyncProgressSummary
  planRowsByInstType?: Partial<Record<InstType, PlanRow[]>>
}

export type PlanRow = {
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  status: 'ok' | 'partial' | 'missing' | 'queued' | 'running' | 'failed'
  label: string
  policyLabel: string
  gap_count: number
  start_ts?: number | null
  end_ts?: number | null
}
