import type { ID, InstType, Timeframe } from './common'

export interface ScannerCondition {
  id: string
  name: string
  description: string
  params: Record<string, unknown>
}

export interface ScannerProfile {
  id: ID
  name: string
  conditions: string[]
  symbol_filter: string[]
  inst_type: InstType
  timeframe: Timeframe
  created_at: string
}

export interface ScannerResult {
  id: ID
  profile_id: ID
  symbol: string
  matched_conditions: string[]
  score: number
  details: Record<string, unknown>
  scanned_at: string
}
