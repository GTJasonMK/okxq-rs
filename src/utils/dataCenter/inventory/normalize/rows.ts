import type { InstType } from '@/types'
import type {
  InventoryMarket,
  InventoryRow,
} from '@/types/dataCenter'
import {
  booleanValue,
  normalizeInputSymbol,
  normalizeInstType,
  normalizeNumberRecord,
  numberValue,
  recordFrom,
  stringValue,
} from '@/utils/dataCenter/normalize'
import { normalizeInventoryMarket } from './markets'

export function normalizeInventoryRow(value: Record<string, unknown>): InventoryRow {
  const markets = recordFrom(value.markets)
  const normalizedMarkets: Partial<Record<InstType, InventoryMarket>> = {}
  for (const [key, market] of Object.entries(markets)) {
    const instType = normalizeInstType(key)
    if (!instType) continue
    const normalizedMarket = normalizeInventoryMarket(market)
    if (normalizedMarket) normalizedMarkets[instType] = normalizedMarket
  }
  const symbol = normalizeInputSymbol(stringValue(value.symbol))
  return {
    symbol,
    base_ccy: stringValue(value.base_ccy, symbol.split('-')[0] ?? ''),
    managed: booleanValue(value.managed),
    watched: booleanValue(value.watched),
    orphan: booleanValue(value.orphan),
    candle_count: numberValue(value.candle_count),
    timeframe_record_count: numberValue(value.timeframe_record_count),
    storage_counts: normalizeNumberRecord(value.storage_counts),
    markets: normalizedMarkets,
  }
}
