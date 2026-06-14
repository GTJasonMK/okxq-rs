export interface RiskSnapshot {
  mode: string
  date: string
  total_equity: number
  spot_value: number
  contract_value: number
  cash_value: number
  positions: Record<string, unknown>
  metadata: Record<string, unknown>
  created_at: string
}

export interface RiskMetrics {
  has_data: boolean
  message: string
  data_points: number | null
  var_95: number | null
  var_99: number | null
  parametric_var_95: number | null
  sharpe_ratio: number | null
  sortino_ratio: number | null
  max_drawdown: number | null
  max_drawdown_duration: number | null
  current_drawdown: number | null
  peak_equity: number | null
  latest_equity: number | null
}

export interface DrawdownSeries {
  dates: string[]
  equities: number[]
  max_drawdown: number | null
  max_drawdown_duration: number | null
  current_drawdown: number | null
  peak: number | null
  series: Array<{ time: number; value: number }>
}

export interface RollingMetrics {
  dates: string[]
  sharpe: number[]
  volatility: number[]
  var_95: number[]
}

export interface RollingMetricSummary {
  name: string
  mean: number
  min_val: number
  max_val: number
  current: number
}

export interface RollingSummary {
  metrics: RollingMetricSummary[]
  benchmark: Array<{ time: number; value: number }>
}

export interface RiskOverview {
  snapshots: RiskSnapshot[]
  metrics: RiskMetrics
  drawdown: DrawdownSeries
  rolling: RollingMetrics & RollingSummary
}
