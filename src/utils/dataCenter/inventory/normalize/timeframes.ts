import { normalizeTimeframe } from '@/api/marketNormalize'
import type {
  InventoryTimeframeRecord,
} from '@/types/dataCenter'
import {
  booleanValue,
  numberValue,
  ratioValue,
  recordFrom,
  stringValue,
  timestampFromValue,
  toNullableString,
} from '@/utils/dataCenter/normalize'

export function normalizeInventoryTimeframe(value: unknown): InventoryTimeframeRecord | null {
  const raw = recordFrom(value)
  const timeframe = normalizeTimeframe(stringValue(raw.timeframe))
  if (!timeframe) return null
  return {
    timeframe,
    managed: booleanValue(raw.managed),
    candle_count: numberValue(raw.candle_count),
    expected_candle_count: numberValue(raw.expected_candle_count),
    gap_count: numberValue(raw.gap_count),
    coverage_ratio: ratioValue(raw.coverage_ratio),
    history_complete: booleanValue(raw.history_complete),
    last_sync_mode: toNullableString(raw.last_sync_mode),
    last_sync_time: toNullableString(raw.last_sync_time),
    oldest_timestamp: timestampFromValue(raw.oldest_timestamp, raw.oldest_time),
    newest_timestamp: timestampFromValue(raw.newest_timestamp, raw.newest_time),
    oldest_time: toNullableString(raw.oldest_time),
    newest_time: toNullableString(raw.newest_time),
  }
}
