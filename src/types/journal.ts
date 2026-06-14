import type { ID, TradingMode } from './common'

export interface JournalEntry {
  id: ID
  entry_id: ID
  title: string
  content: string
  mode: TradingMode
  inst_id: string
  inst_type: string
  trade_ids: string[]
  order_ids: string[]
  tags: string[]
  strategy_id: string
  strategy_name: string
  rating: number
  emotion: string
  screenshots: string[]
  pnl_snapshot: number
  metadata: Record<string, unknown>
  created_at: string
  updated_at: string
}

export interface JournalTag {
  tag: string
  usage_count: number
  color?: string
  created_at?: string
}

export interface JournalStatsGroup {
  key: string
  count: number
  total_pnl: number
  win_rate: number
  avg_rating?: number
}

export interface JournalStats {
  total_entries: number
  group_by: string
  groups: JournalStatsGroup[]
}
