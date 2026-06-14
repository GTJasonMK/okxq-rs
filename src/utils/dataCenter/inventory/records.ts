import type { InstType, SyncRecord } from '@/types'
import type {
  InventoryMarket,
  InventoryRow,
} from '@/types/dataCenter'

export function inventoryRowsToSyncRecords(rows: InventoryRow[]): SyncRecord[] {
  const records: SyncRecord[] = []
  for (const row of rows) {
    for (const market of inventoryMarkets(row)) {
      for (const timeframe of market.timeframes) {
        records.push({
          inst_id: market.inst_id,
          inst_type: market.inst_type,
          timeframe: timeframe.timeframe,
          last_sync_time: timeframe.last_sync_time ?? null,
          oldest_timestamp: timeframe.oldest_timestamp ?? null,
          newest_timestamp: timeframe.newest_timestamp ?? null,
          oldest_time: timeframe.oldest_time ?? null,
          newest_time: timeframe.newest_time ?? null,
          candle_count: timeframe.candle_count,
          expected_candle_count: timeframe.expected_candle_count,
          gap_count: timeframe.gap_count,
          coverage_ratio: timeframe.coverage_ratio,
          history_complete: timeframe.history_complete,
          last_sync_mode: timeframe.last_sync_mode ?? '',
        })
      }
    }
  }
  return records
}

export function inventoryMarkets(row: InventoryRow): InventoryMarket[] {
  return ['SPOT', 'SWAP', 'FUTURES']
    .map(instType => row.markets[instType as InstType])
    .filter((market): market is InventoryMarket => Boolean(market))
}
