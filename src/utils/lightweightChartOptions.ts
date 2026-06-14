import { CrosshairMode, type createChart, type Time } from 'lightweight-charts'
import type { Timeframe } from '@/types'
import { formatChartTimeLabel } from '@/utils/chartTime'

type LightweightChartOptions = NonNullable<Parameters<typeof createChart>[1]>

interface DarkFinancialChartOptions {
  crosshairAlpha?: number
}

export function darkFinancialChartOptions(
  timeframe: Timeframe,
  options: DarkFinancialChartOptions = {},
): LightweightChartOptions {
  const crosshairAlpha = options.crosshairAlpha ?? 0.4
  return {
    layout: {
      background: { color: '#030304' },
      textColor: '#94a3b8',
      fontSize: 11,
      fontFamily: "'Inter', 'Segoe UI', sans-serif",
      attributionLogo: false,
    },
    localization: {
      locale: 'zh-CN',
      timeFormatter: (time: Time) => formatChartTimeLabel(time, timeframe),
    },
    grid: {
      vertLines: { color: 'rgba(255, 255, 255, 0.04)' },
      horzLines: { color: 'rgba(255, 255, 255, 0.04)' },
    },
    rightPriceScale: {
      borderColor: 'rgba(255, 255, 255, 0.06)',
      scaleMargins: { top: 0.08, bottom: 0.22 },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
      vertLine: {
        color: `rgba(247, 147, 26, ${crosshairAlpha})`,
        style: 3,
        labelBackgroundColor: '#f7931a',
      },
      horzLine: {
        color: `rgba(247, 147, 26, ${crosshairAlpha})`,
        style: 3,
        labelBackgroundColor: '#f7931a',
      },
    },
  }
}
