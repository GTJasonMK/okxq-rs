import { describe, expect, it } from 'vitest'
import { isRecord } from '@/api/normalize'
import { normalizeTimeframe as normalizeMarketTimeframe } from '@/utils/marketView'

describe('API 数据归一化', () => {
  it('isRecord 只接受普通对象', () => {
    expect(isRecord({ id: 'direct' })).toBe(true)
    expect(isRecord(Object.create(null))).toBe(true)
    expect(isRecord(null)).toBe(false)
    expect(isRecord([])).toBe(false)
    expect(isRecord('value')).toBe(false)
  })

  it('行情页周期归一化支持小写小时和日线入口', () => {
    expect(normalizeMarketTimeframe('1h')).toBe('1H')
    expect(normalizeMarketTimeframe('4h')).toBe('4H')
    expect(normalizeMarketTimeframe('1d')).toBe('1D')
    expect(normalizeMarketTimeframe('1H')).toBe('1H')
  })
})
