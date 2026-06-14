import type { ID, Timestamp, InstType, Timeframe, OrderSide } from './common'

export interface Candle {
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  timestamp: Timestamp
  open: number
  high: number
  low: number
  close: number
  volume: number
  volume_ccy?: number
  volume_quote?: number
}

export interface Ticker {
  inst_id: string
  inst_type: InstType
  last: number
  ask: number
  bid: number
  open24h: number
  high24h: number
  low24h: number
  vol24h: number
  change24h: number
  ts: Timestamp
}

export interface Orderbook {
  inst_id: string
  bids: Array<{ price: number; size: number; count: number }>
  asks: Array<{ price: number; size: number; count: number }>
  ts: Timestamp
}

export interface RecentTrade {
  inst_id: string
  trade_id: string
  price: number
  size: number
  side: OrderSide
  ts: Timestamp
}

export interface MarketSymbol {
  symbol: string
  base_ccy?: string
  inst_id: string
  inst_type: InstType
  timeframes: Timeframe[]
  candle_count: number
  managed: boolean
  watched: boolean
}

export interface WatchedSymbolSyncPlan {
  timeframe: Timeframe
  enabled: boolean
  bootstrap_days: number
  archive_mode: 'rolling' | 'full'
}

export interface WatchedSymbol {
  symbol: string
  base_ccy?: string
  spot_inst_id: string
  swap_inst_id: string
  sync_spot: boolean
  sync_swap: boolean
  archive_all_history?: boolean
  sync_days?: number
  sync_plans?: WatchedSymbolSyncPlan[]
  created_at?: string
  updated_at?: string
}

export interface SyncJob {
  task_id: ID
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  source_timeframe?: Timeframe
  target_timeframes?: Timeframe[]
  mode: string
  status: string
  progress: number
  message?: string
  days?: number
  start_ts?: Timestamp | null
  end_ts?: Timestamp | null
  repair_method?: string
  reused_existing?: boolean
  saved_count?: number
  fetched_count?: number
  target_fetch_count?: number
  target_save_count?: number
  inserted_count?: number
  derived_count?: number
  target_derive_count?: number
  batches?: number
  target_batches?: number
  api_calls?: number
  candle_count?: number
  history_complete?: boolean
  updated_at?: string
  finished_at?: string | null
  error?: string
  created_at: string
}

export interface SyncRecord {
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  last_sync_time?: string | null
  oldest_timestamp?: Timestamp | null
  newest_timestamp?: Timestamp | null
  oldest_time?: string | null
  newest_time?: string | null
  candle_count: number
  expected_candle_count?: number
  gap_count?: number
  coverage_ratio?: number
  history_complete: boolean
  last_sync_mode?: string
}

type MarketGapRepairMethod = 'paginated' | 'historical_zip'

export interface MarketGapPlanRequest {
  inst_id: string
  inst_type?: InstType
  timeframe: Timeframe
  start_ts?: Timestamp
  end_ts?: Timestamp
  days?: number
  limit?: number
}

export type MarketGapRepairRequest = MarketGapPlanRequest & {
  method?: 'auto' | MarketGapRepairMethod
}

interface MarketGapPlanTimeRange {
  start_ts: Timestamp
  end_ts: Timestamp
  start_time?: string | null
  end_time?: string | null
}

interface MarketGapPlanLocalRange {
  oldest_timestamp?: Timestamp | null
  newest_timestamp?: Timestamp | null
  oldest_time?: string | null
  newest_time?: string | null
}

interface MarketGapPlanZipSource {
  provider: string
  module: string
  date_aggr_type: 'daily' | 'monthly'
  source_timeframe: Timeframe
}

export interface MarketGapRangePlan {
  start_ts: Timestamp
  end_ts: Timestamp
  start_time?: string | null
  end_time?: string | null
  span_ms: number
  missing_candles: number
  method: MarketGapRepairMethod
  reason: string
  fetch_timeframe: Timeframe
  target_timeframes: Timeframe[]
  requires_derivation: boolean
  zip?: MarketGapPlanZipSource | null
}

export interface MarketGapPlan {
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  source_timeframe: Timeframe
  target_timeframes: Timeframe[]
  range: MarketGapPlanTimeRange
  local_range: MarketGapPlanLocalRange
  expected_candles: number
  available_candles: number
  missing_candles: number
  coverage_ratio: number
  gap_event_count: number
  returned_gap_count: number
  returned_missing_candles: number
  truncated: boolean
  max_internal_gap_ms: number
  methods: {
    paginated_ranges: number
    historical_zip_ranges: number
  }
  gaps: MarketGapRangePlan[]
}

export interface SyncRuntimeSettings {
  max_sync_batches: number
  okx_page_pause_ms: number
  sync_job_concurrency: number
  window_fetch_concurrency: number
  window_fetch_batches_per_slice: number
  candle_upsert_transaction_chunk: number
  okx_max_concurrency: number
  okx_public_rest_concurrency: number
  okx_private_rest_concurrency: number
  okx_trade_rest_concurrency: number
  okx_ws_control_concurrency: number
  okx_unknown_concurrency: number
}

type SyncRuntimeLimits = Partial<Record<keyof SyncRuntimeSettings, {
  min: number
  max: number
}>>

export interface SyncRuntimeConfig {
  settings: SyncRuntimeSettings
  defaults: SyncRuntimeSettings
  limits: SyncRuntimeLimits
  active_sync_jobs: number
  message?: string
}

export interface PriceAlert {
  id: ID
  inst_id: string
  symbol?: string
  inst_type: InstType
  alert_type: 'price' | 'change'
  direction: 'above' | 'below'
  target_price?: number | null
  change_percent?: number | null
  note?: string
  enabled: boolean
  trigger_once: boolean
  cooldown_seconds: number
  updated_at?: string
  triggered_at?: string | null
  last_value?: number | null
  last_trigger_value?: number | null
  last_trigger_ts?: number
  created_at: string
}
