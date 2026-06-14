import type { LiveEquityHistory } from '@/types'
import type { LiveEquitySnapshot } from '@/utils/liveStrategyCore/types'
import {
  finiteOrZero,
  validPositiveTimestamp,
} from '@/utils/liveStrategyCore/sort'

export function compareEquitySnapshotsByTime(
  left: LiveEquitySnapshot,
  right: LiveEquitySnapshot,
) {
  return equitySnapshotTimestamp(left) - equitySnapshotTimestamp(right)
    || finiteOrZero(left.created_at) - finiteOrZero(right.created_at)
    || finiteOrZero(left.id) - finiteOrZero(right.id)
}

export function equitySnapshotTimestamp(snapshot: LiveEquitySnapshot): number {
  if (validPositiveTimestamp(snapshot.timestamp)) return snapshot.timestamp
  if (validPositiveTimestamp(snapshot.created_at)) return snapshot.created_at
  return 0
}

export function isValidEquitySnapshot(snapshot: LiveEquitySnapshot): boolean {
  return equitySnapshotTimestamp(snapshot) > 0 && Number.isFinite(snapshot.equity)
}

export function dailySummariesFromSnapshots(
  snapshots: LiveEquityHistory['snapshots'],
): LiveEquityHistory['daily'] {
  const byDay = new Map<string, LiveEquityHistory['snapshots']>()
  for (const snapshot of snapshots) {
    if (!isValidEquitySnapshot(snapshot)) continue
    const day = snapshot.trading_day || '未知日期'
    const rows = byDay.get(day) ?? []
    rows.push(snapshot)
    byDay.set(day, rows)
  }
  return Array.from(byDay.entries()).map(([tradingDay, rows]) => {
    const sorted = [...rows].sort(compareEquitySnapshotsByTime)
    const first = sorted[0]
    const last = sorted[sorted.length - 1]
    return {
      trading_day: tradingDay,
      start_timestamp: first ? equitySnapshotTimestamp(first) : 0,
      end_timestamp: last ? equitySnapshotTimestamp(last) : 0,
      start_time: first?.time ?? '',
      end_time: last?.time ?? '',
      snapshot_count: sorted.length,
      first_equity: first?.equity ?? 0,
      last_equity: last?.equity ?? 0,
      day_start_equity: first?.day_start_equity ?? 0,
      today_pnl: last?.today_pnl ?? null,
      today_pnl_pct: last?.today_pnl_pct ?? null,
      total_pnl: last?.total_pnl ?? null,
      total_pnl_pct: last?.total_pnl_pct ?? null,
      realized_pnl: last?.realized_pnl ?? null,
      unrealized_pnl: last?.unrealized_pnl ?? null,
    }
  })
}
