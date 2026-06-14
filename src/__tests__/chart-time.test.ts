import { describe, expect, it } from 'vitest'
import { TickMarkType, type UTCTimestamp } from 'lightweight-charts'
import {
  formatChartCandleTime,
  formatChartTickMark,
  formatChartTimeLabel,
} from '@/utils/chartTime'

describe('chart time formatting', () => {
  const utc1500 = Date.UTC(2026, 4, 28, 15, 0)

  it('用北京时间显示 OKX UTC 毫秒时间戳', () => {
    expect(formatChartCandleTime(utc1500, '15m')).toBe('05-28 23:00')
    expect(formatChartTimeLabel((utc1500 / 1000) as UTCTimestamp, '15m')).toBe('05-28 23:00')
  })

  it('坐标轴刻度也用北京时间', () => {
    expect(formatChartTickMark((utc1500 / 1000) as UTCTimestamp, TickMarkType.Time)).toBe('23:00')
    expect(formatChartTickMark((utc1500 / 1000) as UTCTimestamp, TickMarkType.DayOfMonth)).toBe('05-28')
  })

  it('日线以上显示北京时间日期', () => {
    expect(formatChartCandleTime(utc1500, '1D')).toBe('2026-05-28')
  })
})
