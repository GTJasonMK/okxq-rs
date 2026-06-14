import type { EquityCandle } from '@/utils/strategyExecution'
import type { Legend } from '../types'
import { formatMoneyValue } from '../format'

export function equityLegend(candle: EquityCandle | null | undefined): Legend | null {
  if (!candle) return null
  const change = equityCandleReturnPct(candle)
  return {
    time: new Date(candle.timestamp).toLocaleString('zh-CN', { hour12: false }),
    open: formatMoneyValue(candle.open),
    high: formatMoneyValue(candle.high),
    low: formatMoneyValue(candle.low),
    close: formatMoneyValue(candle.close),
    change: `${change >= 0 ? '+' : ''}${change.toFixed(2)}%`,
    count: candle.snapshot_count,
    positive: candle.close >= candle.open,
  }
}

export function equityCandleReturnPct(candle: EquityCandle | null | undefined) {
  if (!candle || !Number.isFinite(candle.open) || candle.open <= 0 || !Number.isFinite(candle.close)) {
    return 0
  }
  return (candle.close / candle.open - 1) * 100
}
