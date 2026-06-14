import { describe, expect, it } from 'vitest'
import {
  okxNullableNumberValue,
  okxNumberValue,
  okxPositiveNumberValue,
  okxStringValue,
  okxTimestampValue,
} from '@/api/okxPayload'
import { numberValue } from '@/api/normalize'

describe('OKX payload 读取工具', () => {
  it('只在 OKX 专用 helper 中解析字符串数字', () => {
    expect(okxNumberValue('12.5')).toBe(12.5)
    expect(okxNullableNumberValue('-3')).toBe(-3)
    expect(okxTimestampValue('1700000000000')).toBe(1_700_000_000_000)
    expect(okxNumberValue('bad', 7)).toBe(7)
    expect(okxNullableNumberValue('bad')).toBeNull()

    expect(numberValue('12.5', 7)).toBe(7)
  })

  it('读取 OKX 数字 ID 和正数行情字段', () => {
    expect(okxStringValue(12345)).toBe('12345')
    expect(okxStringValue(false, 'fallback')).toBe('fallback')
    expect(okxPositiveNumberValue('0')).toBeNull()
    expect(okxPositiveNumberValue('-1')).toBeNull()
    expect(okxPositiveNumberValue('155.5')).toBe(155.5)
  })
})
