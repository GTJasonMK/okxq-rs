import type { ID, Timestamp, OrderSide, InstType, Timeframe } from './common'
import type { Candle } from './market'
import type { LiveOrder } from './live-strategy'

type BacktestTradeSide = OrderSide | 'funding' | ''

export interface StrategyMeta {
  id: string
  name: string
  description: string
  strategy_type?: string
  data_requirements?: Record<string, unknown>
  runtime?: StrategyRuntimeConfig
  visualization?: Record<string, unknown>
  decision_contract?: Record<string, unknown>
}

export interface StrategyRuntimeConfig {
  symbol: string
  inst_type: InstType
  timeframe: Timeframe
  risk_timeframe?: Timeframe
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  check_interval?: number
  mode?: string
  params?: Record<string, unknown>
}

export interface BacktestTrade {
  symbol?: string
  timestamp: Timestamp
  datetime: string
  entry_time: string
  exit_time: string
  side: BacktestTradeSide
  action: string
  pos_side: string
  price: number
  entry_price: number | null
  exit_price: number | null
  quantity: number
  base_quantity?: number
  exchange_quantity?: number
  value: number
  commission: number
  pnl: number
  pnl_pct: number
  funding: number
  equity: number
  reason: string
}

export interface BacktestEquitySnapshot {
  time: Timestamp
  equity: number
  cash: number | null
  position_value: number | null
  position_notional: number | null
  unrealized_pnl: number | null
  position_side: string
  leverage: number
  positions?: BacktestPositionSnapshot[]
}

export interface BacktestPositionSnapshot {
  symbol: string
  side: string
  inst_type: string
  timeframe: Timeframe | string
  entry_price: number | null
  quantity: number | null
  exchange_quantity?: number | null
  entry_timestamp: Timestamp | null
  entry_notional: number | null
  entry_reason: string
  reason: string
  stop_loss: number | null
  take_profit: number | null
  planned_exit_time: Timestamp | null
  planned_exit_reason: string
  planned_hold_bars: number | null
  mark_price: number | null
  mark_price_source?: string
  mark_price_missing?: boolean
  notional: number | null
  position_notional: number | null
  unrealized_pnl: number | null
  unrealized_pnl_pct: number | null
}

export interface BacktestResult {
  result_id: ID
  strategy_id: string
  strategy_name: string
  symbol: string
  inst_type: InstType
  timeframe: Timeframe
  days: number
  initial_capital: number
  final_equity: number
  total_return_pct: number
  sharpe_ratio: number
  max_drawdown_pct: number
  win_rate_pct: number
  total_trades: number
  winning_trades: number
  losing_trades: number
  profit_factor: number
  trades: BacktestTrade[]
  orders: LiveOrder[]
  fills: Array<Record<string, unknown>>
  rejected_orders: Array<Record<string, unknown>>
  funding_events?: Array<Record<string, unknown>>
  trade_events_total: number
  trades_truncated: boolean
  candles: Candle[]
  indicators: Record<string, unknown>
  params?: Record<string, unknown>
  strategy_actions?: Array<Record<string, unknown>>
  strategy_diagnostics?: Record<string, unknown>
  runtime_action_summary?: Record<string, unknown>
  execution_model?: Record<string, unknown>
  cost_model?: Record<string, unknown>
  rejected_actions?: Array<Record<string, unknown>>
  strategy_context_stamp?: Record<string, unknown>
  runtime_execution_stamp?: Record<string, unknown>
  backtest_result_integrity?: Record<string, unknown>
  runtime_action_backtest?: boolean
  contract_mode?: boolean
  equity_curve: Array<{ time: Timestamp; equity: number }>
  equity_snapshots?: BacktestEquitySnapshot[]
  created_at: string
}

export interface BacktestProgress {
  run_id: string
  strategy_id: string
  status: 'idle' | 'running' | 'completed' | 'failed'
  stage: string
  message: string
  progress: number
  processed_candles: number
  total_candles: number
  strategy_progress?: Record<string, unknown>
  started_at: string
  updated_at: string
}

interface MonteCarloResult {
  num_simulations: number
  sampling_method: string
  block_size: number
  original_final_equity: number
  original_max_drawdown: number
  equity_percentiles: Array<Record<string, number>>
  drawdown_percentiles: Array<Record<string, number>>
  mean_final_equity: number
  std_final_equity: number
  median_final_equity: number
  prob_profit: number
  prob_original_beat: number
  worst_case_equity: number
  best_case_equity: number
}

export interface BacktestMonteCarloOptions {
  num_simulations?: number
  block_size?: number
}

export interface BacktestMonteCarloResponse {
  result_id: number
  num_trades: number
  initial_capital: number
  analysis: MonteCarloResult
}
