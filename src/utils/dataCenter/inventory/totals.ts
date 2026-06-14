import type { InventorySummary } from '@/types/dataCenter'

export function buildInventoryTableTotals(summary: InventorySummary) {
  return Object.entries(summary.table_totals ?? {})
    .filter(([, value]) => value > 0)
    .map(([key, value]) => ({ key, value }))
    .sort((left, right) => {
      if (left.key === 'total') return -1
      if (right.key === 'total') return 1
      return right.value - left.value
    })
}
