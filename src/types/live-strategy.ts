import type { InstType, Timestamp, TradingMode, Timeframe } from './common'

export interface LiveStrategyStatus {
  status: string
  running: boolean
  run_id: string
  strategy_id: string
  strategy_name: string
  symbol: string
  timeframe: Timeframe
  inst_type: InstType
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  params: Record<string, unknown>
  risk_timeframe: Timeframe
  mode: TradingMode
  start_time: string | null
  last_action_time: string | null
  last_action: string
  actions_generated: number | null
  orders_placed: number | null
  successful_orders: number | null
  failed_orders: number | null
  error_message: string
  check_interval: number
  execution_mode: string
  last_price: number | null
  last_action_strength: number | null
  last_action_reason: string
  last_order_candle_ts: Timestamp | null
}

export interface LiveExecutionLogEntry {
  seq: number
  run_id: string
  mode: TradingMode | ''
  strategy_id: string
  strategy_name: string
  symbol: string
  inst_type: InstType | ''
  timeframe: Timeframe | ''
  timestamp_ms: Timestamp
  time: string
  stage: string
  level: 'info' | 'warn' | 'error' | 'success' | ''
  message: string
  details: Record<string, unknown>
}

export interface LiveOrder {
  id: number
  ord_id: string
  client_order_id: string
  parent_order_id: string
  parent_client_order_id: string
  actual_order_id: string
  actual_client_order_id: string
  inst_id: string
  symbol: string
  order_type: string
  side: string
  sz: number | null
  px: number | null
  reference_price: number | null
  reference_price_source: string
  reference_price_missing: boolean
  fill_count: number
  filled_size: number | null
  filled_quantity: number | null
  avg_fill_price: number | null
  fill_notional: number | null
  remaining_size: number | null
  total_fee: number | null
  fee_ccy: string | null
  first_fill_ts: Timestamp | null
  last_fill_ts: Timestamp | null
  fill_source: string
  action: string
  success: boolean
  status: string
  error_message: string
  mode: TradingMode
  strategy_id: string
  strategy_name: string
  run_id: string
  timestamp: Timestamp | null
  arrival_ts: Timestamp | null
  arrival_mid_px: number | null
  arrival_bid_px: number | null
  arrival_ask_px: number | null
  created_at: Timestamp
}

export interface LiveExecutionPlan {
  id: number
  plan_key: string
  strategy_id: string
  strategy_name: string
  mode: TradingMode
  entry_run_id: string
  exit_run_id: string
  symbol: string
  inst_id: string
  inst_type: InstType
  timeframe: Timeframe
  entry_order_id: string
  entry_client_order_id: string
  entry_timestamp: Timestamp | null
  entry_side: string
  entry_price: number | null
  close_side: string
  planned_exit_time: Timestamp | null
  planned_exit_reason: string
  planned_exit_contract: string
  status: string
  exit_order_id: string
  exit_client_order_id: string
  attempt_count: number
  next_attempt_at: Timestamp | null
  last_error: string
  created_at: Timestamp
  updated_at: Timestamp
}

export interface LiveEquitySnapshot {
  id: number
  run_id: string
  strategy_id: string
  strategy_name: string
  symbol: string
  inst_id: string
  timeframe: Timeframe
  inst_type: InstType
  mode: TradingMode
  timestamp: Timestamp
  time: string
  trading_day: string
  price: number | null
  position_side: string
  entry_price: number | null
  quantity: number | null
  initial_capital: number
  day_start_equity: number
  equity: number
  realized_pnl: number | null
  unrealized_pnl: number | null
  total_pnl: number | null
  total_pnl_pct: number | null
  today_pnl: number | null
  today_pnl_pct: number | null
  created_at: Timestamp
  pnl_available?: boolean
  source?: string
}

export interface LiveEquityDailySummary {
  trading_day: string
  start_timestamp: Timestamp
  end_timestamp: Timestamp
  start_time: string
  end_time: string
  snapshot_count: number
  first_equity: number
  last_equity: number
  day_start_equity: number
  today_pnl: number | null
  today_pnl_pct: number | null
  total_pnl: number | null
  total_pnl_pct: number | null
  realized_pnl: number | null
  unrealized_pnl: number | null
  pnl_available?: boolean
}

export interface LiveEquityHistory {
  run_id: string
  mode: TradingMode
  count: number
  snapshots: LiveEquitySnapshot[]
  daily: LiveEquityDailySummary[]
  pnl_available?: boolean
  source?: string
}

export interface LiveExecutionGate {
  key: string
  label: string
  status: 'pass' | 'block' | 'wait' | 'skip' | 'monitor' | 'info' | ''
  passed: boolean
  blocking: boolean
  detail: string
}

export interface LiveStrategyAction {
  action: string
  symbol: string
  side: string
  order_type: string
  price: number | null
  reference_price: number | null
  trigger_price: number | null
  price_source: string
  reason: string
  strength: number | null
  timestamp: Timestamp
  position_size: number | null
  exchange_size: string
  order_side: string
  close_side: string
  planned_exit_time: Timestamp | null
  planned_exit_reason: string
  planned_exit_contract: string
  order_id: string
  client_order_id: string
  new_size: string
  new_price: string
  request_id: string
  cancel_on_fail: boolean
  target_order_kind: string
  target_order_type: string
  source_index: number | null
  source_time: Timestamp | null
  feature_bar_time: Timestamp | null
  entry_time: Timestamp | null
  planned_hold_bars: number | null
  hold_bars: number | null
  layer_id: string
  family: string
  action_timeframe: string
  candidate_source: string
  candidate_entry_price: number | null
  raw: Record<string, unknown>
}

export interface LiveDecisionActionSummary {
  open_position: number
  close_position: number
  place_risk_order: number
  cancel_order: number
  modify_order: number
  hold: number
  total: number
}

export interface LiveDecisionExecutionLog {
  stage: string
  level: 'info' | 'warn' | 'error' | 'success' | ''
  message: string
  details: Record<string, unknown>
}

export interface LiveDecisionExecutionPreview {
  verdict: 'ready' | 'blocked' | 'hold' | 'preview' | 'mismatch' | 'mixed' | ''
  summary: string
  executable_intent_count: number
  risk_action_count: number
  skipped_action_count: number
  idle_action_count: number
  skipped_actions: LiveStrategyAction[]
  gates: LiveExecutionGate[]
}

export interface LiveDecisionDiagnostics {
  strategy_id: string
  strategy_name: string
  symbol: string
  inst_type: InstType
  timeframe: Timeframe
  summary: string
  candle_count: number | null
  realtime_candle_applied: boolean
  decision_protocol: string
  actions: LiveStrategyAction[]
  action_summary: LiveDecisionActionSummary
  execution_logs: LiveDecisionExecutionLog[]
  selected_symbols: string[]
  blocked_by: string[]
  execution_decision?: LiveDecisionExecutionPreview
  raw: Record<string, unknown>
}
