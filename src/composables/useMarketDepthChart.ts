import { computed, ref } from 'vue'
import type { Orderbook } from '@/types'
import { formatPrice } from '@/utils/format'
import { sortedOrderbookSide } from '@/utils/marketView'

const MAX_DEPTH_ROWS = 5000
const CHART_WIDTH = 320
const CHART_HEIGHT = 180
const CHART_PADDING = { top: 10, right: 10, bottom: 12, left: 10 }
const CHART_CENTER_X = CHART_WIDTH / 2
const CHART_CENTER_GAP = 22
const chartInnerHeight = CHART_HEIGHT - CHART_PADDING.top - CHART_PADDING.bottom

type DepthLevel = { price: number; size: number; cumulative: number }
type ChartPoint = { x: number; y: number }

type DepthHover = {
  x: number
  labelX: number
  labelWidth: number
  price: number
  text: string
}

type DepthChart = {
  hasData: boolean
  bidLinePoints: string
  askLinePoints: string
  bidAreaPoints: string
  askAreaPoints: string
  minPrice: number
  maxPrice: number
  bestBid: number | null
  bestAsk: number | null
  midPrice: number | null
  midX: number | null
  bidTotal: number
  askTotal: number
  bidDepthPct: number
}

type DepthAnalysis = {
  spread: number | null
  depthChart: DepthChart
}

type MarketDepthChartOptions = {
  orderbook: () => Orderbook | null
  bidDepth: () => number
  askDepth: () => number
}

const EMPTY_DEPTH_CHART: DepthChart = {
  hasData: false,
  bidLinePoints: '',
  askLinePoints: '',
  bidAreaPoints: '',
  askAreaPoints: '',
  minPrice: 0,
  maxPrice: 0,
  bestBid: null,
  bestAsk: null,
  midPrice: null,
  midX: null,
  bidTotal: 0,
  askTotal: 0,
  bidDepthPct: 0,
}

const EMPTY_DEPTH_ANALYSIS: DepthAnalysis = {
  spread: null,
  depthChart: EMPTY_DEPTH_CHART,
}

export function useMarketDepthChart(options: MarketDepthChartOptions) {
  const gridLines = [0.25, 0.5, 0.75].map(ratio => CHART_PADDING.top + chartInnerHeight * ratio)
  const bidFillId = `rt-depth-bid-${Math.random().toString(36).slice(2)}`
  const askFillId = `rt-depth-ask-${Math.random().toString(36).slice(2)}`
  const depthSvgRef = ref<SVGSVGElement | null>(null)
  const depthHover = ref<DepthHover | null>(null)

  const depthAnalysis = computed<DepthAnalysis>(() => {
    const orderbook = options.orderbook()
    if (!orderbook) return EMPTY_DEPTH_ANALYSIS

    const rawBids = sortedOrderbookSide(orderbook.bids, 'bid')
    const rawAsks = sortedOrderbookSide(orderbook.asks, 'ask')
    const topBid = rawBids[0]?.price ?? null
    const topAsk = rawAsks[0]?.price ?? null
    const spread = topBid !== null && topAsk !== null && topBid < topAsk ? topAsk - topBid : null
    if (topBid === null || topAsk === null) return { spread, depthChart: EMPTY_DEPTH_CHART }

    const bestAsk = rawAsks.find(row => row.price > topBid)?.price ?? topAsk
    const bestBid = rawBids.find(row => row.price < bestAsk)?.price ?? topBid
    if (bestBid === null || bestAsk === null || bestBid >= bestAsk) {
      return { spread, depthChart: EMPTY_DEPTH_CHART }
    }

    const sortedBids = takeDepthRows(rawBids, clampDepth(options.bidDepth()), row => row.price <= bestBid)
    const sortedAsks = takeDepthRows(rawAsks, clampDepth(options.askDepth()), row => row.price >= bestAsk)
    const depthChart = buildDepthChart(sortedBids, sortedAsks, bestBid, bestAsk)
    return { spread, depthChart }
  })

  const spread = computed(() => depthAnalysis.value.spread)
  const depthChart = computed(() => depthAnalysis.value.depthChart)

  function onDepthPointerMove(event: PointerEvent) {
    const svg = depthSvgRef.value
    if (!svg || !depthChart.value.hasData) {
      clearDepthHover()
      return
    }

    const rect = svg.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) {
      clearDepthHover()
      return
    }

    const rawX = ((event.clientX - rect.left) / rect.width) * CHART_WIDTH
    const x = clamp(rawX, CHART_PADDING.left, CHART_WIDTH - CHART_PADDING.right)
    const price = priceFromChartX(x)
    if (price === null) {
      clearDepthHover()
      return
    }

    const text = formatPrice(price)
    const labelWidth = Math.max(48, text.length * 6.8 + 12)
    depthHover.value = {
      x,
      labelX: clamp(x, labelWidth / 2 + 4, CHART_WIDTH - labelWidth / 2 - 4),
      labelWidth,
      price,
      text,
    }
  }

  function clearDepthHover() {
    depthHover.value = null
  }

  function priceFromChartX(x: number): number | null {
    const chart = depthChart.value
    if (!chart.hasData || chart.bestBid === null || chart.bestAsk === null) return null

    const left = CHART_PADDING.left
    const right = CHART_WIDTH - CHART_PADDING.right
    const bidRight = CHART_CENTER_X - CHART_CENTER_GAP / 2
    const askLeft = CHART_CENTER_X + CHART_CENTER_GAP / 2

    if (x <= bidRight) {
      const ratio = normalizeRatio((x - left) / (bidRight - left))
      return chart.minPrice + ratio * (chart.bestBid - chart.minPrice)
    }
    if (x >= askLeft) {
      const ratio = normalizeRatio((x - askLeft) / (right - askLeft))
      return chart.bestAsk + ratio * (chart.maxPrice - chart.bestAsk)
    }

    const ratio = normalizeRatio((x - bidRight) / (askLeft - bidRight))
    return chart.bestBid + ratio * (chart.bestAsk - chart.bestBid)
  }

  return {
    CHART_WIDTH,
    CHART_HEIGHT,
    CHART_PADDING,
    gridLines,
    bidFillId,
    askFillId,
    depthSvgRef,
    depthHover,
    spread,
    depthChart,
    onDepthPointerMove,
    clearDepthHover,
  }
}

function clampDepth(value: number) {
  return Math.max(1, Math.min(MAX_DEPTH_ROWS, Math.round(Number.isFinite(value) ? value : 1)))
}

function cumulativeLevels(rows: Array<{ price: number; size: number }>): DepthLevel[] {
  let cumulative = 0
  return rows.map(row => {
    cumulative += row.size
    return { price: row.price, size: row.size, cumulative }
  })
}

function takeDepthRows(
  rows: Array<{ price: number; size: number }>,
  limit: number,
  predicate: (row: { price: number; size: number }) => boolean,
) {
  const selected: Array<{ price: number; size: number }> = []
  for (const row of rows) {
    if (!predicate(row)) continue
    selected.push(row)
    if (selected.length >= limit) break
  }
  return selected
}

function buildDepthChart(
  sortedBids: Array<{ price: number; size: number }>,
  sortedAsks: Array<{ price: number; size: number }>,
  bestBid: number,
  bestAsk: number,
): DepthChart {
  if (sortedBids.length === 0 || sortedAsks.length === 0) return EMPTY_DEPTH_CHART
  const bidLevels = cumulativeLevels(sortedBids).reverse()
  const askLevels = cumulativeLevels(sortedAsks)
  if (bidLevels.length === 0 || askLevels.length === 0) return EMPTY_DEPTH_CHART

  let minPrice = bidLevels[0].price
  for (const level of bidLevels) minPrice = Math.min(minPrice, level.price)
  let maxPrice = askLevels[0].price
  for (const level of askLevels) maxPrice = Math.max(maxPrice, level.price)
  if (minPrice === maxPrice) {
    minPrice -= Math.max(1, minPrice * 0.001)
    maxPrice += Math.max(1, maxPrice * 0.001)
  }

  const bidTotal = bidLevels[0]?.cumulative ?? 0
  const askTotal = askLevels[askLevels.length - 1]?.cumulative ?? 0
  let maxCumulative = Math.max(1, bidTotal, askTotal)
  for (const level of bidLevels) maxCumulative = Math.max(maxCumulative, level.cumulative)
  for (const level of askLevels) maxCumulative = Math.max(maxCumulative, level.cumulative)
  const bidChartLevels = pixelBucketDepthLevels(bidLevels, 'bid', minPrice, bestBid)
  const askChartLevels = pixelBucketDepthLevels(askLevels, 'ask', bestAsk, maxPrice)
  const midPrice = (bestBid + bestAsk) / 2
  const bidPoints = stepPoints(
    bidChartLevels.map(level => chartPoint(level, 'bid', minPrice, bestBid, maxCumulative))
  )
  const askPoints = stepPoints(
    askChartLevels.map(level => chartPoint(level, 'ask', bestAsk, maxPrice, maxCumulative))
  )
  const total = bidTotal + askTotal

  return {
    hasData: true,
    bidLinePoints: pointsToString(bidPoints),
    askLinePoints: pointsToString(askPoints),
    bidAreaPoints: areaPoints(bidPoints),
    askAreaPoints: areaPoints(askPoints),
    minPrice,
    maxPrice,
    bestBid,
    bestAsk,
    midPrice,
    midX: CHART_CENTER_X,
    bidTotal,
    askTotal,
    bidDepthPct: total > 0 ? Math.round((bidTotal / total) * 100) : 0,
  }
}

function pixelBucketDepthLevels(
  levels: DepthLevel[],
  side: 'bid' | 'ask',
  minPrice: number,
  maxPrice: number,
) {
  if (levels.length <= 2) return levels
  const selected: DepthLevel[] = []
  let lastBucket: number | null = null
  for (const level of levels) {
    const bucket = Math.round(chartX(level.price, side, minPrice, maxPrice))
    if (bucket === lastBucket) {
      selected[selected.length - 1] = level
    } else {
      selected.push(level)
      lastBucket = bucket
    }
  }
  if (selected[0] !== levels[0]) selected.unshift(levels[0])
  const last = levels[levels.length - 1]
  if (selected[selected.length - 1] !== last) selected.push(last)
  return selected
}

function chartPoint(
  level: DepthLevel,
  side: 'bid' | 'ask',
  minPrice: number,
  maxPrice: number,
  maxCumulative: number
): ChartPoint {
  return {
    x: chartX(level.price, side, minPrice, maxPrice),
    y: chartY(level.cumulative, maxCumulative),
  }
}

function chartX(price: number, side: 'bid' | 'ask', minPrice: number, maxPrice: number) {
  const left = CHART_PADDING.left
  const right = CHART_WIDTH - CHART_PADDING.right
  const bidRight = CHART_CENTER_X - CHART_CENTER_GAP / 2
  const askLeft = CHART_CENTER_X + CHART_CENTER_GAP / 2
  const range = maxPrice - minPrice
  const ratio = range > 0 ? (price - minPrice) / range : side === 'bid' ? 1 : 0
  if (side === 'bid') return left + ratio * (bidRight - left)
  return askLeft + ratio * (right - askLeft)
}

function chartY(cumulative: number, maxCumulative: number) {
  return CHART_HEIGHT - CHART_PADDING.bottom - (cumulative / maxCumulative) * chartInnerHeight
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value))
}

function normalizeRatio(value: number) {
  return clamp(Number.isFinite(value) ? value : 0, 0, 1)
}

function pointsToString(points: ChartPoint[]) {
  return points.map(point => `${point.x.toFixed(1)},${point.y.toFixed(1)}`).join(' ')
}

function stepPoints(points: ChartPoint[]): ChartPoint[] {
  if (points.length <= 1) return points
  const stepped: ChartPoint[] = [points[0]]
  for (let i = 1; i < points.length; i += 1) {
    stepped.push({ x: points[i].x, y: points[i - 1].y }, points[i])
  }
  return stepped
}

function areaPoints(points: ChartPoint[]) {
  if (points.length === 0) return ''
  const baseY = CHART_HEIGHT - CHART_PADDING.bottom
  const first = points[0]
  const last = points[points.length - 1]
  return [
    `${first.x.toFixed(1)},${baseY}`,
    pointsToString(points),
    `${last.x.toFixed(1)},${baseY}`,
  ].join(' ')
}
