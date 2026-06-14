import type * as T from '@/types/risk'
import {
  arrayRecords,
  arrayValue,
  booleanValue,
  isRecord,
  nullableNumberValue,
  recordFrom,
  stringValue,
} from '../normalize'

type AnyRecord = Record<string, unknown>

export function normalizeSnapshot(raw: AnyRecord): T.RiskSnapshot | null {
  const totalEquity = riskNumber(raw.total_equity)
  const spotValue = riskNumber(raw.spot_value)
  const contractValue = riskNumber(raw.contract_value)
  const cashValue = riskNumber(raw.cash_value)
  if (totalEquity === null || spotValue === null || contractValue === null || cashValue === null) return null
  return {
    mode: stringValue(raw.mode, 'simulated'),
    date: stringValue(raw.date),
    total_equity: totalEquity,
    spot_value: spotValue,
    contract_value: contractValue,
    cash_value: cashValue,
    positions: isRecord(raw.positions) ? raw.positions : {},
    metadata: isRecord(raw.metadata) ? raw.metadata : {},
    created_at: normalizeDateString(raw.created_at),
  }
}

export function normalizeMetrics(raw: unknown): T.RiskMetrics {
  const data = recordFrom(raw)
  return {
    has_data: booleanValue(data.has_data),
    message: stringValue(data.message),
    data_points: nullableNumberValue(data.data_points),
    var_95: nullableNumberValue(data.var_95),
    var_99: nullableNumberValue(data.var_99),
    parametric_var_95: nullableNumberValue(data.parametric_var_95),
    sharpe_ratio: nullableNumberValue(data.sharpe_ratio),
    sortino_ratio: nullableNumberValue(data.sortino_ratio),
    max_drawdown: nullableNumberValue(data.max_drawdown),
    max_drawdown_duration: nullableNumberValue(data.max_drawdown_duration),
    current_drawdown: nullableNumberValue(data.current_drawdown),
    peak_equity: nullableNumberValue(data.peak_equity),
    latest_equity: nullableNumberValue(data.latest_equity),
  }
}

export function normalizeDrawdown(raw: unknown): T.DrawdownSeries {
  const data = recordFrom(raw)
  const dates = stringSeries(data.dates)
  return {
    dates,
    equities: numericSeries(data.equities),
    max_drawdown: nullableNumberValue(data.max_drawdown),
    max_drawdown_duration: nullableNumberValue(data.max_drawdown_duration),
    current_drawdown: nullableNumberValue(data.current_drawdown),
    peak: nullableNumberValue(data.peak),
    series: numericPoints(data.dates, data.series),
  }
}

export function normalizeRolling(raw: unknown): T.RollingMetrics & T.RollingSummary {
  const data = recordFrom(raw)
  const dates = stringSeries(data.dates)
  const sharpe = numericSeries(data.sharpe)
  const volatility = numericSeries(data.volatility)
  const var95 = numericSeries(data.var_95)
  const benchmark = numericPoints(data.dates, data.sharpe)
  return {
    dates,
    sharpe,
    volatility,
    var_95: var95,
    metrics: [
      summarize('Sharpe', sharpe),
      summarize('波动率', volatility),
      summarize('VaR 95%', var95),
    ].filter((item): item is T.RollingMetricSummary => item !== null),
    benchmark,
  }
}

export function normalizeOverview(raw: unknown): T.RiskOverview {
  const data = recordFrom(raw)
  return {
    snapshots: arrayRecords(data.snapshots)
      .map(normalizeSnapshot)
      .filter((snapshot): snapshot is T.RiskSnapshot => snapshot !== null),
    metrics: normalizeMetrics(data.metrics),
    drawdown: normalizeDrawdown(data.drawdown),
    rolling: normalizeRolling(data.rolling),
  }
}

function summarize(name: string, values: number[]): T.RollingMetricSummary | null {
  const valid = values.filter(Number.isFinite)
  if (valid.length === 0) return null
  return {
    name,
    mean: valid.reduce((sum, value) => sum + value, 0) / valid.length,
    min_val: Math.min(...valid),
    max_val: Math.max(...valid),
    current: valid[valid.length - 1],
  }
}

function toChartTime(value: unknown): number {
  const dateText = stringValue(value).trim()
  if (!dateText) throw new Error('风险图表日期缺失')
  const parsed = Date.parse(dateText)
  if (!Number.isFinite(parsed)) throw new Error(`风险图表日期无效: ${dateText}`)
  return Math.floor(parsed / 1000)
}

function numericSeries(value: unknown): number[] {
  return arrayValue(value).filter(numberFilter)
}

function normalizeDateString(value: unknown): string {
  return stringValue(value).trim()
}

function riskNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function stringSeries(value: unknown): string[] {
  return arrayValue(value).filter(stringFilter)
}

function numericPoints(dates: unknown, values: unknown) {
  const dateRows = arrayValue(dates)
  return arrayValue(values)
    .map((value, index) => {
      if (!numberFilter(value)) return null
      return { time: toChartTime(dateRows[index]), value }
    })
    .filter((item): item is { time: number; value: number } => item !== null)
}

function numberFilter(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function stringFilter(value: unknown): value is string {
  return typeof value === 'string'
}
