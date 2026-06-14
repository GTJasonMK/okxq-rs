import type { Candle, Timeframe } from '@/types'
import { formatChartCandleTime } from '@/utils/chartTime'
import type { RealtimeTriggerCandle } from './types'

export function liveTriggerSubtitleText(input: {
  symbol: string
  timeframe: Timeframe
  latestCandle: Candle | null
  latestRealtimeCandle: RealtimeTriggerCandle | null
  markerCount: number
}) {
  const symbol = input.symbol || '未选择品种'
  const latestText = input.latestCandle
    ? `最新K线 ${formatChartCandleTime(input.latestCandle.timestamp, input.timeframe)} · ${triggerCandleFreshnessText(input.latestRealtimeCandle, input.latestCandle)}`
    : '暂无K线'
  return `${symbol} · ${input.timeframe} · ${latestText} · ${input.markerCount} 个动作点`
}

export function triggerCandleFreshnessText(
  latestRealtimeCandle: RealtimeTriggerCandle | null,
  candle: Candle,
) {
  if (
    !latestRealtimeCandle ||
    latestRealtimeCandle.inst_id !== candle.inst_id ||
    latestRealtimeCandle.timeframe !== candle.timeframe ||
    latestRealtimeCandle.timestamp !== candle.timestamp
  ) return '历史数据'
  return latestRealtimeCandle.confirm === '1' ? '已收盘' : '实时更新'
}
