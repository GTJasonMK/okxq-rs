<template>
  <div class="equity-candle-chart">
    <div ref="containerRef" class="ecc-chart"></div>
    <EquityCandleOverlays
      :active-histogram-metric="activeHistogramMetric"
      :legend="legend"
      :range-summary="rangeSummary"
      :title="title"
      @select-histogram-metric="setActiveHistogramMetric"
    />
    <EquityCandleTooltip v-if="hoverTooltip" :tooltip="hoverTooltip" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import {
  CandlestickSeries,
  HistogramSeries,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import type { BacktestEquitySnapshot, BacktestTrade, Timeframe } from '@/types'
import type { EquityCandle } from '@/utils/strategyExecution'
import { CHART_COLORS } from '@/utils/color'
import { formatChartTimeLabel } from '@/utils/chartTime'
import { darkFinancialChartOptions } from '@/utils/lightweightChartOptions'
import {
  sortedEquitySnapshots,
  sortedEquityCandles,
  sortedTradeEvents,
} from '@/utils/strategyExecution'
import {
  chartTimeSecond,
  equityHoverTooltip,
  equityLegend,
  equityRangeSummary,
  equityStats,
  equityHistogramValues,
  formatMoneyValue,
  type EquityHistogramMetric,
  type EquityHistogramPoint,
  type HoverTooltip,
  type Legend,
  type RangeSummary,
} from '@/utils/equityCandleChart'
import {
  createResponsiveChart,
  useLightweightChartLifecycle,
  type LightweightChartApi,
} from '@/composables/useLightweightChartLifecycle'
import EquityCandleOverlays from '@/components/strategy/EquityCandleOverlays.vue'
import EquityCandleTooltip from '@/components/strategy/EquityCandleTooltip.vue'

const props = defineProps<{
  candles: EquityCandle[]
  snapshots?: BacktestEquitySnapshot[]
  timeframe: Timeframe
  title?: string
  trades?: BacktestTrade[]
}>()

type ChartApi = LightweightChartApi
type SeriesApi = ReturnType<ChartApi['addSeries']>

const containerRef = ref<HTMLDivElement>()
const legend = ref<Legend | null>(null)
const rangeSummary = ref<RangeSummary | null>(null)
const hoverTooltip = ref<HoverTooltip | null>(null)
const activeHistogramMetric = ref<EquityHistogramMetric>('return_pct')
let chart: ChartApi | null = null
let candleSeries: SeriesApi | null = null
let qualitySeries: SeriesApi | null = null
let maxPriceLine: ReturnType<SeriesApi['createPriceLine']> | null = null
let minPriceLine: ReturnType<SeriesApi['createPriceLine']> | null = null
let currentCandles: EquityCandle[] = []
let currentSnapshots: BacktestEquitySnapshot[] = []
let currentTrades: BacktestTrade[] = []
let candleIndexBySecond = new Map<number, number>()
let crosshairFrameId: number | null = null
let pendingCrosshair: { second: number; x: number; y: number } | null = null

function initChart(element: HTMLDivElement) {
  destroyChart()

  chart = createResponsiveChart(element, {
    ...darkFinancialChartOptions(props.timeframe, { crosshairAlpha: 0.42 }),
    timeScale: {
      borderColor: 'rgba(255,255,255,0.06)',
      timeVisible: true,
      secondsVisible: false,
      rightOffset: 14,
      barSpacing: 8,
      minBarSpacing: 2,
    },
    handleScroll: {
      mouseWheel: true,
      pressedMouseMove: true,
      horzTouchDrag: true,
      vertTouchDrag: true,
    },
    handleScale: {
      mouseWheel: true,
      pinch: true,
      axisPressedMouseMove: { time: true, price: true },
      axisDoubleClickReset: { time: true, price: true },
    },
  })

  candleSeries = chart.addSeries(CandlestickSeries, {
    upColor: CHART_COLORS.positive,
    downColor: CHART_COLORS.negative,
    borderUpColor: CHART_COLORS.positive,
    borderDownColor: CHART_COLORS.negative,
    wickUpColor: CHART_COLORS.positive,
    wickDownColor: CHART_COLORS.negative,
  })

  qualitySeries = chart.addSeries(HistogramSeries, {
    base: 0,
    priceFormat: {
      type: 'custom',
      formatter: (value: number) => `${value.toFixed(2)}%`,
    },
    priceScaleId: 'quality',
  })
  chart.priceScale('quality').applyOptions({ scaleMargins: { top: 0.82, bottom: 0 } })

  chart.subscribeCrosshairMove((param) => {
    if (!param.point || !param.time || param.point.x < 0 || param.point.y < 0) {
      hideHoverTooltip()
      return
    }
    const second = chartTimeSecond(param.time)
    if (!second) {
      hideHoverTooltip()
      return
    }
    scheduleCrosshairUpdate(second, param.point.x, param.point.y)
  })

  refreshSnapshotRows()
  refreshTradeRows()
  setData(props.candles)
}

function setData(candles: EquityCandle[]) {
  refreshSnapshotRows()
  refreshTradeRows()
  currentCandles = sortedEquityCandles(candles, { copy: false })
  candleIndexBySecond = new Map(currentCandles.map((candle, index) => [
    Math.floor(candle.timestamp / 1000),
    index,
  ]))
  candleSeries?.setData(currentCandles.map(candle => ({
    time: Math.floor(candle.timestamp / 1000) as UTCTimestamp,
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
  })))
  updateHistogramSeries()
  updateExtremes()
  updateLegend(currentCandles.length - 1)
  chart?.timeScale().fitContent()
}

function updateHistogramSeries() {
  qualitySeries?.setData(equityHistogramValues({
    candles: currentCandles,
    snapshots: currentSnapshots,
    timeframe: props.timeframe,
    metric: activeHistogramMetric.value,
    sorted: true,
  }).map(point => ({
    time: Math.floor(point.timestamp / 1000) as UTCTimestamp,
    value: point.value,
    color: histogramColor(point),
  })))
}

function refreshSnapshotRows() {
  currentSnapshots = sortedEquitySnapshots(props.snapshots ?? [])
}

function refreshTradeRows() {
  currentTrades = sortedTradeEvents(props.trades ?? [])
}

function updateLegendFromTime(time: number) {
  const index = candleIndexBySecond.get(time) ?? -1
  updateLegend(index >= 0 ? index : currentCandles.length - 1)
}

function scheduleCrosshairUpdate(second: number, x: number, y: number) {
  pendingCrosshair = { second, x, y }
  if (crosshairFrameId !== null) return
  crosshairFrameId = window.requestAnimationFrame(() => {
    crosshairFrameId = null
    const next = pendingCrosshair
    if (!next) return
    updateLegendFromTime(next.second)
    updateHoverTooltip(next.second, next.x, next.y)
  })
}

function updateLegend(index: number) {
  legend.value = equityLegend(currentCandles[index])
}

function updateHoverTooltip(second: number, pointX: number, pointY: number) {
  const index = candleIndexBySecond.get(second) ?? -1
  const candle = index >= 0 ? currentCandles[index] : null
  if (!candle) {
    hideHoverTooltip()
    return
  }

  hoverTooltip.value = equityHoverTooltip({
    candle,
    snapshots: currentSnapshots,
    trades: currentTrades,
    timeframe: props.timeframe,
    pointX,
    pointY,
    containerWidth: containerRef.value?.clientWidth ?? 0,
    containerHeight: containerRef.value?.clientHeight ?? 0,
  })
}

function updateExtremes() {
  clearExtremePriceLines()
  const stats = equityStats(currentSnapshots, currentCandles, { sorted: true })
  if (!stats) {
    rangeSummary.value = null
    return
  }

  rangeSummary.value = equityRangeSummary(stats)

  if (!candleSeries) return
  maxPriceLine = candleSeries.createPriceLine({
    price: stats.max.value,
    color: CHART_COLORS.positive,
    lineWidth: 1,
    lineStyle: 2,
    axisLabelVisible: true,
    title: `最高 ${formatMoneyValue(stats.max.value)}`,
  })
  minPriceLine = candleSeries.createPriceLine({
    price: stats.min.value,
    color: CHART_COLORS.negative,
    lineWidth: 1,
    lineStyle: 2,
    axisLabelVisible: true,
    title: `最低 ${formatMoneyValue(stats.min.value)}`,
  })
}

function clearExtremePriceLines() {
  if (candleSeries && maxPriceLine) candleSeries.removePriceLine(maxPriceLine)
  if (candleSeries && minPriceLine) candleSeries.removePriceLine(minPriceLine)
  maxPriceLine = null
  minPriceLine = null
}

function hideHoverTooltip() {
  pendingCrosshair = null
  hoverTooltip.value = null
}

function setActiveHistogramMetric(metric: EquityHistogramMetric) {
  activeHistogramMetric.value = metric
}

function histogramColor(point: EquityHistogramPoint) {
  if (activeHistogramMetric.value === 'exposure_pct') {
    if (point.value <= 0) return 'rgba(148,163,184,0.24)'
    if (point.side === 'short') return 'rgba(239,83,80,0.44)'
    if (point.side === 'portfolio') return 'rgba(41,98,255,0.42)'
    return 'rgba(38,166,154,0.44)'
  }
  if (activeHistogramMetric.value === 'drawdown_pressure_pct') {
    return point.value < 0 ? 'rgba(239,83,80,0.44)' : 'rgba(148,163,184,0.22)'
  }
  return point.value >= 0 ? 'rgba(38,166,154,0.42)' : 'rgba(239,83,80,0.42)'
}

function destroyChart() {
  clearExtremePriceLines()
  if (crosshairFrameId !== null) {
    window.cancelAnimationFrame(crosshairFrameId)
    crosshairFrameId = null
  }
  chart?.remove()
  chart = null
  candleSeries = null
  qualitySeries = null
  currentCandles = []
  currentSnapshots = []
  currentTrades = []
  candleIndexBySecond = new Map()
  pendingCrosshair = null
  legend.value = null
  rangeSummary.value = null
  hoverTooltip.value = null
}

watch(() => props.candles, (candles) => {
  hideHoverTooltip()
  setData(candles)
})

watch(() => props.snapshots, () => {
  refreshSnapshotRows()
  updateHistogramSeries()
  updateExtremes()
})

watch(() => props.trades, () => {
  refreshTradeRows()
  hideHoverTooltip()
})

watch(() => props.timeframe, () => {
  if (!chart) return
  hideHoverTooltip()
  chart.applyOptions({
    localization: {
      locale: 'zh-CN',
      timeFormatter: (time: Time) => formatChartTimeLabel(time, props.timeframe),
    },
  })
  setData(props.candles)
})

watch(activeHistogramMetric, () => {
  updateHistogramSeries()
})

useLightweightChartLifecycle({
  containerRef,
  chart: () => chart,
  initChart,
  destroyChart,
})

</script>

<style scoped>
.equity-candle-chart {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  contain: layout paint;
}
.ecc-chart {
  width: 100%;
  height: 100%;
  min-height: 0;
}
</style>
