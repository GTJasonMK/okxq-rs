import type {
  InventoryMarket,
  InventoryTimeframeRecord,
} from '@/types/dataCenter'
import { formatCount, formatDateTimeValue, formatOptionalDateTime } from '@/utils/dataCenter/format'
import { formatRatio } from '@/utils/dataCenter/normalize'

export function inventoryMarketSummary(market: InventoryMarket) {
  const latest = formatDateTimeValue(market.newest_time ?? market.newest_timestamp)
  const oldest = formatDateTimeValue(market.oldest_time ?? market.oldest_timestamp)
  const gap = market.gap_count > 0 ? ` · 缺失 ${formatCount(market.gap_count)}` : ' · 无缺失'
  return `范围 ${oldest} 至 ${latest}${gap}`
}

export function inventoryMarketGapLabel(market: InventoryMarket) {
  return market.gap_count > 0 ? `缺失 ${formatCount(market.gap_count)}` : '无缺失'
}

export function inventoryTimeframeRangeLabel(record: InventoryTimeframeRecord) {
  const oldest = formatOptionalDateTime(record.oldest_time ?? record.oldest_timestamp)
  const newest = formatOptionalDateTime(record.newest_time ?? record.newest_timestamp)
  if (oldest && newest) return `${oldest} 至 ${newest}`
  if (newest) return `最新 ${newest}`
  if (oldest) return `起始 ${oldest}`
  return '范围 --'
}

export function inventoryTimeframeGapLabel(record: InventoryTimeframeRecord) {
  return record.gap_count > 0 ? `缺失 ${formatCount(record.gap_count)}` : '无缺失'
}

export function inventoryTimeframeCoverageLabel(record: InventoryTimeframeRecord) {
  if (record.expected_candle_count <= 0) return '覆盖 --'
  return `覆盖 ${formatRatio(record.coverage_ratio)}`
}

export function storageCountLabel(key: string) {
  const labels: Record<string, string> = {
    total: '总记录',
    candles: 'K 线',
    feature_bars_1s: '秒级特征柱',
    sync_records: '同步记录',
    market_ticker_snapshots: 'Ticker 快照',
    market_recent_trades: '逐笔成交',
    local_fills: '本地成交',
    live_order_records: '实盘订单',
    backtest_results: '回测结果',
    cost_basis: '成本记录',
  }
  return labels[key] ?? key
}
