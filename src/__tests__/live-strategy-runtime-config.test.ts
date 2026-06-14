import { describe, expect, it } from 'vitest'
import { DEFAULT_LIVE_CONTROL_FORM } from '@/utils/liveStrategyControl'
import { liveRuntimeConfigDisabledReason } from '@/utils/liveStrategyRuntimeConfig'

describe('liveStrategyRuntimeConfig', () => {
  it('blocks unsupported strategy timeframe before launch', () => {
    const reason = liveRuntimeConfigDisabledReason({
      form: { ...DEFAULT_LIVE_CONTROL_FORM, timeframe: '1H' },
      supportedTimeframes: ['15m'],
      supportedSymbols: ['BTC-USDT-SWAP'],
    })

    expect(reason).toBe('当前策略不支持周期 1H')
  })

  it('拒绝已删除的本地组合层参数', () => {
    const reason = liveRuntimeConfigDisabledReason({
      form: { ...DEFAULT_LIVE_CONTROL_FORM, params: { portfolio_layers: [{}] } },
      supportedTimeframes: ['15m'],
      supportedSymbols: ['BTC-USDT-SWAP'],
    })

    expect(reason).toContain('portfolio_layers 本地组合架构')
  })

  it('validates numeric runtime fields', () => {
    const reason = liveRuntimeConfigDisabledReason({
      form: { ...DEFAULT_LIVE_CONTROL_FORM, position_size: 2 },
      supportedTimeframes: ['15m'],
      supportedSymbols: ['BTC-USDT-SWAP'],
    })

    expect(reason).toContain('仓位大小')
  })
})
