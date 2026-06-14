export interface Legend {
  time: string
  open: string
  high: string
  low: string
  close: string
  change: string
  count: number
  positive: boolean
}

export interface EquityExtreme {
  timestamp: number
  value: number
}

export interface RangeSummary {
  max: string
  min: string
  range: string
  drawdown: string
}

export interface EquityStats {
  max: EquityExtreme
  min: EquityExtreme
  range: number
  maxDrawdownPct: number
}

export interface HoverTooltipEvent {
  key: string
  time: string
  symbol: string
  label: string
  sideClass: string
  pnl: string
  pnlClass: string
}

export interface HoverTooltipPosition {
  key: string
  symbol: string
  side: string
  sideClass: string
  quantity: string
  entryPrice: string
  markPrice: string
  notional: string
  pnl: string
  pnlClass: string
  returnPct: string
  returnClass: string
}

export interface HoverTooltip {
  x: number
  y: number
  time: string
  open: string
  high: string
  low: string
  close: string
  change: string
  positive: boolean
  equity: string
  cash: string
  notional: string
  unrealized: string
  unrealizedClass: string
  position: string
  positionDetail: string
  positionClass: string
  exposure: string
  leverage: string
  count: number
  positionTitle: string
  positionEmpty: string
  positions: HoverTooltipPosition[]
  positionsTotal: number
  positionsMore: string
  eventTitle: string
  events: HoverTooltipEvent[]
}

export type EquityHistogramMetric = 'return_pct' | 'drawdown_pressure_pct' | 'exposure_pct'

export interface EquityHistogramMetricOption {
  id: EquityHistogramMetric
  label: string
}

export interface EquityHistogramPoint {
  timestamp: number
  value: number
  side: string
}

export type NumericValue = number | null | undefined
