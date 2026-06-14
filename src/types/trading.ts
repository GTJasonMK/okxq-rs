import type { ID, Timestamp, OrderSide, OrderType, PositionSide, InstType } from './common'

export type MarginMode = 'cash' | 'cross' | 'isolated'
export type PositionMode = 'net_mode' | 'long_short_mode'
type NullableNumber = number | null
type NullableTimestamp = Timestamp | null

export interface AccountInfo {
  total_eq: NullableNumber
  iso_eq: NullableNumber
  adj_eq: NullableNumber
  usdt_balance: NullableNumber
  usdt_available: NullableNumber
  usdt_equity_usd: NullableNumber
  details: AccountAsset[]
}

export interface AccountAsset {
  ccy: string
  total: NullableNumber
  available: NullableNumber
  frozen: NullableNumber
  cash_bal: NullableNumber
  avail_bal: NullableNumber
  avail_eq: NullableNumber
  eq: NullableNumber
  eq_usd: NullableNumber
  dis_eq: NullableNumber
  ord_frozen: NullableNumber
  u_time: number
}

export interface ContractAccountConfig {
  pos_mode: PositionMode
  raw: Record<string, unknown>
}

export interface ContractLeverageInfo {
  inst_id: string
  mgn_mode: MarginMode
  pos_side: PositionSide | 'net' | ''
  lever: number
}

export interface Position {
  inst_id: string
  inst_type: InstType
  pos_side: PositionSide | ''
  pos: NullableNumber
  mgn_mode: MarginMode
  avg_px: NullableNumber
  upl: NullableNumber
  upl_ratio: NullableNumber
  lever: NullableNumber
  margin: NullableNumber
  mark_px: NullableNumber
}

export interface Order {
  ord_id: ID
  inst_id: string
  side: OrderSide
  ord_type: OrderType
  sz: NullableNumber
  px: NullableNumber
  state: string
  fill_sz: NullableNumber
  fill_px: NullableNumber
  avg_px: NullableNumber
  pnl: NullableNumber
  ctime: NullableTimestamp
}

export interface Fill {
  fill_id: ID
  inst_id: string
  ord_id: ID
  side: OrderSide
  fill_px: NullableNumber
  fill_sz: NullableNumber
  fee: NullableNumber
  fee_ccy: string
  fill_time: NullableTimestamp
}

export interface LocalFill {
  id: ID
  inst_id: string
  ccy: string
  side: OrderSide
  quantity: number
  price: number
  fee: NullableNumber
  total_cost: number
  fill_time: string
}

export interface LocalFillsSyncResult {
  mode: string
  inst_type: string
  inst_id: string
  fetched: number
  stored: number
  skipped_missing_trade_id: number
  arrival_matched: number
  note: string
}

export interface CostBasis {
  ccy: string
  total_quantity: number
  total_cost: number
  avg_price: number
  unrealized_pnl: number
}

export interface TradePerformance {
  inst_id: string
  total_trades: number
  win_rate: NullableNumber
  total_pnl: NullableNumber
  profit_factor: NullableNumber
  largest_win: NullableNumber
  largest_loss: NullableNumber
}

export interface RiskControlConfig {
  enabled: boolean
  max_single_loss_ratio: number
  max_position_pct: number
  max_order_value: number
}
