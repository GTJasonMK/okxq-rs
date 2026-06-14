import { onBeforeUnmount, onMounted, type Ref, watch } from 'vue'
import { AreaSeries, createChart } from 'lightweight-charts'

type AreaPoint = { time: number; value: number }
type FitContentMode = 'always' | 'with-data'

interface AreaSeriesChartOptions {
  containerRef: Ref<HTMLDivElement | undefined>
  height: number
  seriesOptions: Record<string, unknown>
  data: () => AreaPoint[]
  watchSource: () => unknown
  fitContent?: FitContentMode
}

export function useAreaSeriesChart(options: AreaSeriesChartOptions) {
  let chart: ReturnType<typeof createChart> | null = null

  function initChart() {
    if (!options.containerRef.value || chart) return
    const element = options.containerRef.value
    chart = createChart(element, {
      width: element.clientWidth,
      height: options.height,
      layout: { background: { color: 'transparent' }, textColor: '#888' },
      grid: {
        vertLines: { color: 'rgba(255,255,255,0.04)' },
        horzLines: { color: 'rgba(255,255,255,0.04)' },
      },
      timeScale: { timeVisible: false, borderColor: 'rgba(255,255,255,0.08)' },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.08)' },
    })
    const series = chart.addSeries(AreaSeries, options.seriesOptions as never)
    const rows = options.data()
    series.setData(rows.map(row => ({ time: row.time as never, value: row.value })))
    if ((options.fitContent ?? 'with-data') === 'always' || rows.length > 0) {
      chart.timeScale().fitContent()
    }
  }

  function destroyChart() {
    chart?.remove()
    chart = null
  }

  onMounted(() => { initChart() })
  onBeforeUnmount(() => { destroyChart() })
  watch(options.watchSource, () => {
    destroyChart()
    initChart()
  })
}
