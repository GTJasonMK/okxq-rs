import { TickMarkType, type Time } from 'lightweight-charts'
import type { Timeframe } from '@/types'

const CHART_TIME_ZONE = 'Asia/Shanghai'

const dateTimePartsFormatter = new Intl.DateTimeFormat('zh-CN', {
  timeZone: CHART_TIME_ZONE,
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
})

type DateTimeParts = {
  year: string
  month: string
  day: string
  hour: string
  minute: string
}

function timeToTimestampMs(time: Time): number {
  if (typeof time === 'number' && Number.isFinite(time)) return time * 1000
  if (typeof time === 'string') {
    const parsed = Date.parse(time)
    return Number.isFinite(parsed) ? parsed : 0
  }
  if (time && typeof time === 'object') {
    const item = time as Partial<{ year: number; month: number; day: number }>
    if (
      Number.isFinite(item.year) &&
      Number.isFinite(item.month) &&
      Number.isFinite(item.day)
    ) {
      return Date.UTC(item.year!, item.month! - 1, item.day!)
    }
  }
  return 0
}

export function formatChartTimeLabel(time: Time, timeframe: Timeframe): string {
  return formatChartCandleTime(timeToTimestampMs(time), timeframe)
}

export function formatChartCandleTime(timestamp: number, timeframe: Timeframe): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--'
  const parts = beijingParts(timestamp)
  if (timeframe === '1D' || timeframe === '1W' || timeframe === '1M') {
    return `${parts.year}-${parts.month}-${parts.day}`
  }
  return `${parts.month}-${parts.day} ${parts.hour}:${parts.minute}`
}

export function formatChartTickMark(time: Time, tickMarkType: TickMarkType): string | null {
  const timestamp = timeToTimestampMs(time)
  if (!Number.isFinite(timestamp) || timestamp <= 0) return null
  const parts = beijingParts(timestamp)
  switch (tickMarkType) {
    case TickMarkType.Year:
      return parts.year
    case TickMarkType.Month:
      return `${parts.year}-${parts.month}`
    case TickMarkType.DayOfMonth:
      return `${parts.month}-${parts.day}`
    case TickMarkType.Time:
    case TickMarkType.TimeWithSeconds:
      return `${parts.hour}:${parts.minute}`
    default:
      return `${parts.month}-${parts.day} ${parts.hour}:${parts.minute}`
  }
}

function beijingParts(timestamp: number): DateTimeParts {
  const values: Partial<DateTimeParts> = {}
  for (const part of dateTimePartsFormatter.formatToParts(new Date(timestamp))) {
    if (
      part.type === 'year' ||
      part.type === 'month' ||
      part.type === 'day' ||
      part.type === 'hour' ||
      part.type === 'minute'
    ) {
      values[part.type] = part.value
    }
  }
  return {
    year: values.year ?? '0000',
    month: values.month ?? '00',
    day: values.day ?? '00',
    hour: values.hour ?? '00',
    minute: values.minute ?? '00',
  }
}
