import type { TickCollectorStatus } from '@/types/dataCenter'
import {
  booleanValue,
  normalizeStringList,
  numberValue,
  recordFrom,
  stringValue,
} from './normalize'

export function normalizeTickCollectorStatus(value: unknown): TickCollectorStatus {
  const raw = recordFrom(value)
  return {
    running: booleanValue(raw.running),
    active_symbols: normalizeStringList(raw.active_symbols),
    book_channel: stringValue(raw.book_channel, 'books5'),
    total_trades_received: numberValue(raw.total_trades_received),
    total_bars_written: numberValue(raw.total_bars_written),
    last_trade_ts: numberValue(raw.last_trade_ts),
    errors: normalizeStringList(raw.errors),
  }
}
