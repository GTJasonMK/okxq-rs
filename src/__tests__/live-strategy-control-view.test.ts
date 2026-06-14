import { describe, expect, it } from 'vitest'
import { DEFAULT_LIVE_CONTROL_FORM } from '@/utils/liveStrategyControl'
import {
  liveActionBusyText,
  liveLaunchReadiness,
  liveRiskScopeNote,
} from '@/utils/liveStrategyControlView'

describe('liveStrategyControlView', () => {
  it('生成启动/停止中的就绪状态', () => {
    expect(liveActionBusyText('starting')).toBe('策略启动中...')

    const readiness = liveLaunchReadiness({
      actionLoading: true,
      actionPhase: 'stopping',
      actionBusyText: liveActionBusyText('stopping'),
      formLocked: false,
      startDisabledReason: '',
      launchMode: 'simulated',
      form: { ...DEFAULT_LIVE_CONTROL_FORM, strategy_id: 'strategy-a' },
    })

    expect(readiness).toEqual({
      kind: 'busy',
      title: '停止中',
      detail: '策略停止中...',
    })
  })

  it('可启动时展示真实启动配置', () => {
    const readiness = liveLaunchReadiness({
      actionLoading: false,
      actionPhase: 'idle',
      actionBusyText: liveActionBusyText('idle'),
      formLocked: false,
      startDisabledReason: '',
      launchMode: 'simulated',
      form: { ...DEFAULT_LIVE_CONTROL_FORM, strategy_id: 'single-strategy' },
    })

    expect(readiness.kind).toBe('ready')
    expect(readiness.detail).toContain('模拟盘')
    expect(readiness.detail).toContain(DEFAULT_LIVE_CONTROL_FORM.symbol)
    expect(readiness.detail).toContain(DEFAULT_LIVE_CONTROL_FORM.timeframe)
    expect(readiness.detail).toContain('资金1.00K')
    expect(readiness.detail).toContain('仓位')
  })

  it('不再显示旧本地组合层风险范围提示', () => {
    expect(liveRiskScopeNote()).toBe('')
  })
})
