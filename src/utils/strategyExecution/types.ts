import type { Timestamp } from '@/types'

export interface EquityCandle {
  timestamp: Timestamp
  open: number
  high: number
  low: number
  close: number
  volume: number
  snapshot_count: number
}
