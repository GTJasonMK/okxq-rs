import { describe, expect, it } from 'vitest'
import {
  currentDecisionDiagnosticsForTarget,
  diagnosticScopeText,
  diagnosticTargetKey,
  latestRealtimeDiagnosticCandle,
  decisionDiagnosticsIsStale,
  decisionDiagnosticsMismatchText,
  shouldRefreshDecisionDiagnosticsOnCandle,
  type LiveStrategyDiagnosticTarget,
} from '@/utils/liveStrategyDiagnostics'
import { decisionDiagnostics } from './fixtures/liveStrategy'

describe('liveStrategyDiagnostics', () => {
  it('只返回当前目标 key 和内容都匹配的决策诊断结果', () => {
    const target = diagnosticTarget()
    const targetKey = diagnosticTargetKey(target)
    const diagnostics = decisionDiagnostics({ strategy_id: 'strategy-a', symbol: 'BTC-USDT-SWAP', timeframe: '15m' })

    expect(currentDecisionDiagnosticsForTarget(
      diagnostics,
      targetKey,
      targetKey,
      target,
    )).not.toBeNull()
    expect(currentDecisionDiagnosticsForTarget(
      decisionDiagnostics({ strategy_id: 'strategy-b', symbol: 'BTC-USDT-SWAP', timeframe: '15m' }),
      targetKey,
      targetKey,
      target,
    )).toBeNull()
    expect(decisionDiagnosticsIsStale(diagnostics, targetKey, targetKey, target)).toBe(false)
    expect(decisionDiagnosticsIsStale(diagnostics, 'old-key', targetKey, target)).toBe(true)
  })

  it('诊断 scope 文案只在没有当前结果时显示加载前缀', () => {
    const target = diagnosticTarget()

    expect(diagnosticScopeText({
      target: { ...target, strategy_id: '' },
      loading: false,
      hasCurrentResult: false,
      emptyText: '选择策略',
      loadingPrefix: '正在评估',
    })).toBe('选择策略')
    expect(diagnosticScopeText({
      target,
      loading: true,
      hasCurrentResult: false,
      emptyText: '选择策略',
      loadingPrefix: '正在评估',
    })).toBe('正在评估 · BTC-USDT-SWAP · 15m · strategy-a')
    expect(diagnosticScopeText({
      target,
      loading: true,
      hasCurrentResult: true,
      emptyText: '选择策略',
      loadingPrefix: '正在评估',
    })).toBe('BTC-USDT-SWAP · 15m · strategy-a')
  })

  it('诊断刷新 gate 和错配文案集中在工具层', () => {
    expect(shouldRefreshDecisionDiagnosticsOnCandle(true, true)).toBe(true)
    expect(shouldRefreshDecisionDiagnosticsOnCandle(true, false)).toBe(false)
    expect(shouldRefreshDecisionDiagnosticsOnCandle(false, true)).toBe(false)
    expect(decisionDiagnosticsMismatchText(decisionDiagnostics({ symbol: 'ETH-USDT-SWAP', timeframe: '1H' }))).toBe(
      '决策结果与当前查看层不一致：返回 ETH-USDT-SWAP · 1H',
    )
  })

  it('实时诊断 K 线要求完整成交量证据', () => {
    const target = diagnosticTarget()
    const latest = latestRealtimeDiagnosticCandle({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      timestamp: 1_780_000_000_000,
      open: 100,
      high: 101,
      low: 99,
      close: 100,
      volume: 12,
      volume_ccy: 1200,
      volume_quote: 1200,
      confirm: '1',
    }, target)

    expect(latest).toMatchObject({
      volume_ccy: 1200,
      volume_quote: 1200,
      confirm: '1',
    })
    expect(latestRealtimeDiagnosticCandle({
      ...latest!,
      volume_ccy: undefined,
    }, target)).toBeNull()
  })
})

function diagnosticTarget(): LiveStrategyDiagnosticTarget {
  return {
    strategy_id: 'strategy-a',
    symbol: 'BTC-USDT-SWAP',
    timeframe: '15m',
    initial_capital: 1000,
    position_size: 0.25,
    stop_loss: 0,
    take_profit: 0,
    mode: 'simulated',
    params: {},
  }
}
