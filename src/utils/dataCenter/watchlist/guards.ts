import type { WatchedSymbol } from '@/types'
import type { WatchedRow } from '@/types/dataCenter'

export function isInventoryOnlyRow(row: WatchedSymbol): row is WatchedRow {
  return (row as WatchedRow).inventory_only === true
}
