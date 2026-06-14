import { ref, watch } from 'vue'
import {
  CandlestickSeries,
  createSeriesMarkers,
  HistogramSeries,
  LineSeries,
  TickMarkType,
  type ISeriesMarkersPluginApi,
  type LogicalRangeChangeEventHandler,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import type { StrategyTriggerMarker, StrategyTriggerMarkerMode } from '@/types/strategy-visualization'
import { formatChartCandleTime, formatChartTickMark } from '@/utils/chartTime'
import { CHART_COLORS } from '@/utils/color'
import { darkFinancialChartOptions } from '@/utils/lightweightChartOptions'
import { candleLimitForRange, latestAnchoredVisibleLogicalRange } from '@/utils/marketView'
import { toChartSeriesMarkers } from '@/utils/strategyTriggers'
import {
  createResponsiveChart,
  useLightweightChartLifecycle,
  type LightweightChartApi,
} from '@/composables/useLightweightChartLifecycle'

type ChartApi = LightweightChartApi
type SeriesApi = ReturnType<ChartApi['addSeries']>

interface KlineChartProps {
  readonly candles: Candle[]
  readonly timeframe: Timeframe
  readonly rangeDays: CandleRangeDays
  readonly markers?: StrategyTriggerMarker[]
  readonly markerMode?: StrategyTriggerMarkerMode
}

type KlineLegend = {
  symbol: string
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
  changePct: string
  positive: boolean
  ma5: string
  ma10: string
}

export function useKlineChart(props: KlineChartProps) {
  const containerRef = ref<HTMLDivElement>()
  const legend = ref<KlineLegend | null>(null)
  let chart: ChartApi | null = null
  let candlestickSeries: SeriesApi | null = null
  let volumeSeries: SeriesApi | null = null
  let ma5Series: SeriesApi | null = null
  let ma10Series: SeriesApi | null = null
  let markersApi: ISeriesMarkersPluginApi<Time> | null = null
  let visibleRangeHandler: LogicalRangeChangeEventHandler | null = null
  let lastDataContext = ''
  let currentCandles: Candle[] = []
  let ma5Values: Array<number | null> = []
  let ma10Values: Array<number | null> = []
  let constraintPending = false

  function chartOptions() {
    const isSmall = props.timeframe === '1m' || props.timeframe === '5m' || props.timeframe === '15m'
    return {
      ...darkFinancialChartOptions(props.timeframe),
      timeScale: {
        timeVisible: isSmall,
        secondsVisible: false,
        borderColor: 'rgba(255, 255, 255, 0.06)',
        rightOffset: 50,
        barSpacing: 8,
        minBarSpacing: 2,
        fixLeftEdge: true,
        fixRightEdge: false,
        lockVisibleTimeRangeOnResize: false,
        shiftVisibleRangeOnNewBar: true,
        tickMarkFormatter: (time: Time, tickMarkType: TickMarkType) =>
          formatChartTickMark(time, tickMarkType),
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
        axisPressedMouseMove: {
          time: true,
          price: true,
        },
        axisDoubleClickReset: {
          time: true,
          price: true,
        },
      },
      kineticScroll: {
        mouse: true,
        touch: true,
      },
    }
  }

  function buildCandleData(candles: Candle[]) {
    return candles.map(candle => ({
      time: (candle.timestamp / 1000) as UTCTimestamp,
      open: candle.open,
      high: candle.high,
      low: candle.low,
      close: candle.close,
    }))
  }

  function buildVolumeData(candles: Candle[]) {
    return candles.map(candle => ({
      time: (candle.timestamp / 1000) as UTCTimestamp,
      value: candle.volume,
      color: candle.close >= candle.open
        ? 'rgba(38, 166, 154, 0.4)'
        : 'rgba(239, 83, 80, 0.4)',
    }))
  }

  function dataContext(candles: Candle[]) {
    const first = candles[0]
    const instId = first?.inst_id || ''
    const instType = first?.inst_type || ''
    return `${instId}:${instType}:${props.timeframe}:${props.rangeDays}`
  }

  function setChartData(candles: Candle[], fitMode: 'auto' | 'force' = 'auto') {
    if (!candlestickSeries || !volumeSeries || !chart) return
    currentCandles = candles
    ma5Values = movingAverage(candles, 5)
    ma10Values = movingAverage(candles, 10)
    candlestickSeries.setData(buildCandleData(candles))
    volumeSeries.setData(buildVolumeData(candles))
    ma5Series?.setData(buildLineData(candles, ma5Values))
    ma10Series?.setData(buildLineData(candles, ma10Values))
    updateMarkers()
    updateLegend(candles.length - 1)
    if (candles.length === 0) {
      lastDataContext = ''
      legend.value = null
      return
    }
    const nextContext = dataContext(candles)
    const shouldFit = fitMode === 'force' || nextContext !== lastDataContext
    lastDataContext = nextContext
    if (shouldFit) applyDefaultVisibleRange(candles)
  }

  function updateMarkers() {
    markersApi?.setMarkers(toChartSeriesMarkers(props.markers ?? [], props.timeframe, props.markerMode))
  }

  function buildLineData(candles: Candle[], values: Array<number | null>) {
    const data: Array<{ time: UTCTimestamp; value: number }> = []
    for (let index = 0; index < candles.length; index += 1) {
      const value = values[index]
      if (value === null || !Number.isFinite(value)) continue
      data.push({
        time: (candles[index].timestamp / 1000) as UTCTimestamp,
        value,
      })
    }
    return data
  }

  function applyDefaultVisibleRange(candles: Candle[]) {
    if (!chart) return
    if (candles.length <= 1) {
      chart.timeScale().fitContent()
      return
    }
    const visibleBars = Math.min(candles.length, candleLimitForRange(props.timeframe, props.rangeDays))
    const range = latestAnchoredVisibleLogicalRange(candles.length, visibleBars)
    if (range) chart.timeScale().setVisibleLogicalRange(range)
  }

  function updateLegend(index: number) {
    const candle = currentCandles[index]
    if (!candle) {
      legend.value = null
      return
    }
    const changePct = candle.open > 0 ? ((candle.close - candle.open) / candle.open) * 100 : 0
    legend.value = {
      symbol: candle.inst_id || '',
      time: formatChartCandleTime(candle.timestamp, props.timeframe),
      open: formatPrice(candle.open),
      high: formatPrice(candle.high),
      low: formatPrice(candle.low),
      close: formatPrice(candle.close),
      volume: formatVolume(candle.volume),
      changePct: `${changePct >= 0 ? '+' : ''}${changePct.toFixed(2)}%`,
      positive: candle.close >= candle.open,
      ma5: formatOptionalPrice(ma5Values[index]),
      ma10: formatOptionalPrice(ma10Values[index]),
    }
  }

  function updateLegendFromTime(time: unknown) {
    const timestamp = typeof time === 'number' ? time * 1000 : 0
    if (!timestamp) {
      updateLegend(currentCandles.length - 1)
      return
    }
    const index = currentCandles.findIndex(candle => Math.floor(candle.timestamp / 1000) === time)
    updateLegend(index >= 0 ? index : currentCandles.length - 1)
  }

  function subscribeLegendUpdates() {
    chart?.subscribeCrosshairMove(param => {
      updateLegendFromTime(param.time)
    })
  }

  function subscribeVisibleRangeConstraint() {
    if (!chart) return
    visibleRangeHandler = (logicalRange) => {
      if (!chart || !logicalRange || currentCandles.length === 0 || constraintPending) return
      const dataLength = currentCandles.length
      const visibleBars = logicalRange.to - logicalRange.from
      if (!Number.isFinite(visibleBars) || visibleBars <= 0) return

      let fixFrom = Number(logicalRange.from)
      let fixTo = Number(logicalRange.to)
      let needFix = false

      const minTo = Math.max(visibleBars * 0.4, 1)
      if (fixTo < minTo) {
        fixTo = minTo
        fixFrom = fixTo - visibleBars
        needFix = true
      }

      const maxRightOffset = visibleBars * 0.6
      const currentRightOffset = fixTo - (dataLength - 1)
      if (currentRightOffset > maxRightOffset) {
        fixTo = dataLength - 1 + maxRightOffset
        fixFrom = fixTo - visibleBars
        needFix = true
      }

      if (!needFix) return
      constraintPending = true
      chart.timeScale().setVisibleLogicalRange({ from: fixFrom, to: fixTo })
      window.requestAnimationFrame(() => {
        constraintPending = false
      })
    }
    chart.timeScale().subscribeVisibleLogicalRangeChange(visibleRangeHandler)
  }

  function initChart(element: HTMLDivElement) {
    destroyChart()

    chart = createResponsiveChart(element, chartOptions())

    candlestickSeries = chart.addSeries(CandlestickSeries, {
      upColor: CHART_COLORS.positive,
      downColor: CHART_COLORS.negative,
      borderUpColor: CHART_COLORS.positive,
      borderDownColor: CHART_COLORS.negative,
      wickUpColor: CHART_COLORS.positive,
      wickDownColor: CHART_COLORS.negative,
    })
    markersApi = createSeriesMarkers(candlestickSeries, [], { autoScale: true, zOrder: 'top' })

    volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    })

    ma5Series = chart.addSeries(LineSeries, {
      color: '#f6c85d',
      lineWidth: 1,
      crosshairMarkerVisible: false,
      priceLineVisible: false,
      lastValueVisible: false,
    })

    ma10Series = chart.addSeries(LineSeries, {
      color: '#6be6c1',
      lineWidth: 1,
      crosshairMarkerVisible: false,
      priceLineVisible: false,
      lastValueVisible: false,
    })

    chart.priceScale('volume').applyOptions({
      scaleMargins: { top: 0.82, bottom: 0 },
    })

    if (props.candles.length > 0) {
      setChartData(props.candles, 'force')
    }
    subscribeLegendUpdates()
    subscribeVisibleRangeConstraint()
  }

  function destroyChart() {
    if (chart && visibleRangeHandler) {
      chart.timeScale().unsubscribeVisibleLogicalRangeChange(visibleRangeHandler)
    }
    visibleRangeHandler = null
    chart?.remove()
    chart = null
    candlestickSeries = null
    volumeSeries = null
    ma5Series = null
    ma10Series = null
    markersApi = null
    lastDataContext = ''
    currentCandles = []
    ma5Values = []
    ma10Values = []
    constraintPending = false
    legend.value = null
  }

  watch(() => props.candles, (data) => {
    setChartData(data)
  })

  watch(() => props.markers, () => {
    updateMarkers()
  })

  watch(() => props.markerMode, () => {
    updateMarkers()
  })

  watch(() => props.timeframe, () => {
    if (!chart) return
    chart.applyOptions(chartOptions())
    setChartData(props.candles, 'force')
  })

  watch(() => props.rangeDays, () => {
    setChartData(props.candles, 'force')
  })

  useLightweightChartLifecycle({
    containerRef,
    chart: () => chart,
    initChart,
    destroyChart,
  })

  return { containerRef, legend }
}

function movingAverage(candles: Candle[], period: number): Array<number | null> {
  const result = new Array<number | null>(candles.length).fill(null)
  let sum = 0
  for (let index = 0; index < candles.length; index += 1) {
    sum += candles[index].close
    if (index >= period) sum -= candles[index - period].close
    if (index >= period - 1) result[index] = sum / period
  }
  return result
}

function formatPrice(value: number) {
  if (!Number.isFinite(value)) return '--'
  if (Math.abs(value) >= 1000) return value.toLocaleString(undefined, { maximumFractionDigits: 2 })
  if (Math.abs(value) >= 1) return value.toLocaleString(undefined, { maximumFractionDigits: 4 })
  return value.toLocaleString(undefined, { maximumFractionDigits: 8 })
}

function formatOptionalPrice(value: number | null) {
  return value === null ? '' : formatPrice(value)
}

function formatVolume(value: number) {
  if (!Number.isFinite(value)) return '--'
  if (Math.abs(value) >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`
  if (Math.abs(value) >= 1_000) return `${(value / 1_000).toFixed(2)}K`
  return value.toLocaleString(undefined, { maximumFractionDigits: 2 })
}
