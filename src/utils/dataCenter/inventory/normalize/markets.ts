import { timeframeOrder } from '@/utils/syncPlans'
import type {
  InventoryMarket,
  InventoryTimeframeRecord,
} from '@/types/dataCenter'
import {
  arrayRecords,
  booleanValue,
  normalizeInstType,
  numberValue,
  recordFrom,
  stringValue,
  timestampFromValue,
  toNullableString,
} from '@/utils/dataCenter/normalize'
import { normalizeInventoryTimeframe } from './timeframes'

export function normalizeInventoryMarket(value: unknown): InventoryMarket | null {
  const raw = recordFrom(value)
  const instType = normalizeInstType(raw.inst_type)
  if (!instType) return null
  const timeframes = arrayRecords(raw.timeframes)
    .map(normalizeInventoryTimeframe)
    .filter((timeframe): timeframe is InventoryTimeframeRecord => !!timeframe)
    .sort((left, right) => timeframeOrder(left.timeframe) - timeframeOrder(right.timeframe))
  return {
    inst_id: stringValue(raw.inst_id),
    inst_type: instType,
    managed: booleanValue(raw.managed),
    watched: booleanValue(raw.watched),
    timeframe_count: numberValue(raw.timeframe_count, timeframes.length),
    candle_count: numberValue(raw.candle_count),
    gap_count: numberValue(raw.gap_count, sumTimeframeGaps(timeframes)),
    history_complete_count: numberValue(raw.history_complete_count, timeframes.filter(item => item.history_complete).length),
    oldest_timestamp: timestampFromValue(raw.oldest_timestamp, raw.oldest_time),
    newest_timestamp: timestampFromValue(raw.newest_timestamp, raw.newest_time),
    oldest_time: toNullableString(raw.oldest_time),
    newest_time: toNullableString(raw.newest_time),
    last_sync_time: toNullableString(raw.last_sync_time),
    timeframes,
  }
}

function sumTimeframeGaps(timeframes: InventoryTimeframeRecord[]) {
  return timeframes.reduce((total, item) => total + Math.max(0, item.gap_count), 0)
}
