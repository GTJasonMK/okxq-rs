import type { ID, TradingMode } from './common'

export interface ChatSession {
  id: ID
  session_id: ID
  title: string
  mode: TradingMode
  created_at: string
}

export interface ChatMessage {
  id: ID
  message_id: ID
  session_id: ID
  role: 'user' | 'assistant' | 'system'
  content: string
  created_at: string
}

export interface OrderDraft {
  id: ID
  draft_id: ID
  session_id?: ID
  inst_id: string
  mode?: TradingMode
  side: string
  order_type: string
  size: string
  price: string
  status: string
  created_at?: string
  updated_at?: string
}

export interface PatrolConfig {
  enabled: boolean
  interval_seconds: number
  interval_minutes: number
  symbols: string[]
  scan_limit?: number
  candidate_limit?: number
  inst_type?: string
  timeframes?: string[]
  candles_limit?: number
  recent_trade_limit?: number
  orderbook_depth?: number
  mode?: TradingMode
  min_priority_score?: number
  notification_cooldown_seconds?: number
}

export interface PatrolRun {
  run_id: ID
  status: string
  candidates: unknown[]
  summary: Record<string, unknown>
  started_at: string | null
  finished_at: string | null
}

export interface LevelSnapshot {
  id: ID
  snapshot_id: ID
  session_id?: ID
  inst_id: string
  mode: TradingMode
  timeframes: unknown[]
  supports: unknown[]
  resistances: unknown[]
  invalidation_levels: unknown[]
  chart_annotations: unknown[]
  summary: Record<string, unknown>
  metadata: Record<string, unknown>
  created_at?: string
}
