import type { Timeframe } from '@/types'
import type { InventoryGapRepairPayload } from '@/types/dataCenter'
import { formatCount } from '@/utils/dataCenter/format'
import { isValidTimestamp } from '@/utils/dataCenter/normalize'

export function inventoryGapKey(
  instId: string,
  instType: InventoryGapRepairPayload['inst_type'],
  timeframe: Timeframe,
) {
  return `${instId}:${instType}:${timeframe}`
}

export function hasValidInventoryGapRange(payload: InventoryGapRepairPayload) {
  return (
    isValidTimestamp(payload.start_ts) &&
    isValidTimestamp(payload.end_ts) &&
    payload.end_ts >= payload.start_ts
  )
}

export function gapRepairMethodLabel(methods: { paginated_ranges?: number; historical_zip_ranges?: number }) {
  const labels: string[] = []
  if ((methods.paginated_ranges ?? 0) > 0) labels.push(`分页 ${formatCount(methods.paginated_ranges ?? 0)} 段`)
  if ((methods.historical_zip_ranges ?? 0) > 0) labels.push(`历史 zip ${formatCount(methods.historical_zip_ranges ?? 0)} 段`)
  return labels.join(' / ')
}
