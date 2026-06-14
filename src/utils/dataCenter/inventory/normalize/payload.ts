import type {
  InventoryRow,
  InventorySummary,
} from '@/types/dataCenter'
import {
  arrayRecords,
  normalizeNumberRecord,
  numberValue,
  recordFrom,
} from '@/utils/dataCenter/normalize'
import { normalizeInventoryRow } from './rows'

export function emptyInventorySummary(): InventorySummary {
  return {
    symbol_count: 0,
    managed_symbol_count: 0,
    managed_market_count: 0,
    watched_symbol_count: 0,
    watched_list_count: 0,
    watched_market_count: 0,
    orphan_symbol_count: 0,
    total_candles: 0,
    total_timeframe_records: 0,
    table_totals: {},
  }
}

export function normalizeInventoryPayload(value: unknown): { summary: InventorySummary; rows: InventoryRow[] } {
  const payload = recordFrom(value)
  const summary = normalizeInventorySummary(payload.summary)
  const rows = arrayRecords(payload.rows).map(normalizeInventoryRow)
  return { summary, rows }
}

function normalizeInventorySummary(value: unknown): InventorySummary {
  const raw = recordFrom(value)
  const tableTotals = normalizeNumberRecord(raw.table_totals)
  return {
    symbol_count: numberValue(raw.symbol_count),
    managed_symbol_count: numberValue(raw.managed_symbol_count),
    managed_market_count: numberValue(raw.managed_market_count),
    watched_symbol_count: numberValue(raw.watched_symbol_count),
    watched_list_count: numberValue(raw.watched_list_count),
    watched_market_count: numberValue(raw.watched_market_count),
    orphan_symbol_count: numberValue(raw.orphan_symbol_count),
    total_candles: numberValue(raw.total_candles),
    total_timeframe_records: numberValue(raw.total_timeframe_records),
    table_totals: tableTotals,
  }
}
