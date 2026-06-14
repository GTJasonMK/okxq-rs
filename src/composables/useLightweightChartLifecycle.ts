import { onMounted, onUnmounted, type Ref } from 'vue'
import { createChart } from 'lightweight-charts'

export type LightweightChartApi = ReturnType<typeof createChart>
export type LightweightChartOptions = NonNullable<Parameters<typeof createChart>[1]>

interface LightweightChartLifecycleOptions {
  containerRef: Ref<HTMLDivElement | undefined>
  chart: () => LightweightChartApi | null
  initChart: (element: HTMLDivElement) => void
  destroyChart: () => void
}

export function createResponsiveChart(
  element: HTMLDivElement,
  options: LightweightChartOptions,
): LightweightChartApi {
  return createChart(element, {
    ...options,
    width: element.clientWidth,
    height: element.clientHeight,
  })
}

export function useLightweightChartLifecycle(options: LightweightChartLifecycleOptions) {
  let resizeObserver: ResizeObserver | null = null

  function disconnectResizeObserver() {
    resizeObserver?.disconnect()
    resizeObserver = null
  }

  function resizeChart() {
    const chart = options.chart()
    const element = options.containerRef.value
    if (!chart || !element) return
    chart.applyOptions({
      width: element.clientWidth,
      height: element.clientHeight,
    })
  }

  function mountChart() {
    const element = options.containerRef.value
    if (!element) return
    disconnectResizeObserver()
    options.initChart(element)
    if (!options.chart()) return
    resizeObserver = new ResizeObserver(resizeChart)
    resizeObserver.observe(element)
  }

  function unmountChart() {
    disconnectResizeObserver()
    options.destroyChart()
  }

  onMounted(mountChart)
  onUnmounted(unmountChart)

  return {
    mountChart,
    resizeChart,
    unmountChart,
  }
}
