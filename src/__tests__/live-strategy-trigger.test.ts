import { describe, expect, it } from 'vitest'
import type { Candle } from '@/types'
import {
  liveTriggerSubtitleText,
  mergeTriggerCandles,
  triggerCandleFreshnessText,
  triggerCandleRequestMatches,
  triggerRangeSelectOptions,
} from '@/utils/liveStrategyTrigger'

describe('liveStrategyTrigger', () => {
  it('动作图 K 线按时间合并、替换并限制行数', () => {
    const merged = mergeTriggerCandles([
      candle({ timestamp: 3, close: 3 }),
      candle({ timestamp: Number.NaN, close: 9 }),
      candle({ timestamp: 1, close: 1 }),
    ], candle({ timestamp: 2, close: 20 }), 2)

    expect(merged.map(item => [item.timestamp, item.close])).toEqual([
      [2, 20],
      [3, 3],
    ])

    const replaced = mergeTriggerCandles(merged, candle({ timestamp: 3, close: 30 }), 10)
    expect(replaced.map(item => [item.timestamp, item.close])).toEqual([
      [2, 20],
      [3, 30],
    ])
  })

  it('动作图 subtitle 展示历史、实时和已收盘状态', () => {
    const latest = candle({ timestamp: Date.parse('2026-05-28T00:00:00.000Z') })

    expect(triggerCandleFreshnessText(null, latest)).toBe('历史数据')
    expect(triggerCandleFreshnessText({ ...latest, confirm: '0' }, latest)).toBe('实时更新')
    expect(triggerCandleFreshnessText({ ...latest, confirm: '1' }, latest)).toBe('已收盘')
    expect(liveTriggerSubtitleText({
      symbol: 'BTC-USDT-SWAP',
      timeframe: '15m',
      latestCandle: latest,
      latestRealtimeCandle: { ...latest, confirm: '1' },
      markerCount: 2,
    })).toContain('BTC-USDT-SWAP · 15m · 最新K线')
    expect(liveTriggerSubtitleText({
      symbol: '',
      timeframe: '15m',
      latestCandle: null,
      latestRealtimeCandle: null,
      markerCount: 0,
    })).toBe('未选择品种 · 15m · 暂无K线 · 0 个动作点')
  })

  it('动作图 range 下拉和请求匹配规则保持纯函数化', () => {
    expect(triggerRangeSelectOptions('15m').map(option => option.value)).toContain('7')
    expect(triggerCandleRequestMatches({
      sequence: 2,
      instId: 'BTC-USDT-SWAP',
      timeframe: '15m',
      rangeDays: 7,
    }, {
      sequence: 2,
      instId: 'BTC-USDT-SWAP',
      timeframe: '15m',
      rangeDays: 7,
    })).toBe(true)
    expect(triggerCandleRequestMatches({
      sequence: 2,
      instId: 'BTC-USDT-SWAP',
      timeframe: '15m',
      rangeDays: 7,
    }, {
      sequence: 3,
      instId: 'BTC-USDT-SWAP',
      timeframe: '15m',
      rangeDays: 7,
    })).toBe(false)
  })
})

function candle(overrides: Partial<Candle> = {}): Candle {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    timestamp: 1,
    open: 1,
    high: 1,
    low: 1,
    close: 1,
    volume: 1,
    ...overrides,
  }
}
