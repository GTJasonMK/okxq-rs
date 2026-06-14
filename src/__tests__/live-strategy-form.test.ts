import { describe, expect, it } from 'vitest'
import type { StrategyMeta } from '@/types'
import { status } from './fixtures/liveStrategy'
import { DEFAULT_LIVE_CONTROL_FORM } from '@/utils/liveStrategyControl'
import {
  applyLiveStrategyFormState,
  enforceStrategyRuntimeInstType,
  enforceStrategyRuntimeSymbol,
  enforceStrategyRuntimeTimeframe,
  liveStrategyFormFromRunningStatus,
  liveStrategyFormFromStrategy,
  strategyRuntimeSymbolOptions,
  strategyRuntimeTimeframeOptions,
  type LiveStrategyControlForm,
} from '@/utils/liveStrategyForm'

describe('liveStrategyForm', () => {
  it('从策略 runtime 生成控制表单并克隆 params', () => {
    const strategy = strategyMeta({
      id: 'strategy-a',
      runtime: {
        symbol: 'ETH-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '1H',
        risk_timeframe: '5m',
        initial_capital: 5000,
        position_size: 0.4,
        stop_loss: 0.03,
        take_profit: 0.1,
        check_interval: 30,
        params: { nested: { value: 1 } },
      },
    })

    const form = liveStrategyFormFromStrategy(strategy, DEFAULT_LIVE_CONTROL_FORM)
    ;(strategy.runtime?.params?.nested as { value: number }).value = 2

    expect(form).toMatchObject({
      strategy_id: 'strategy-a',
      symbol: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      risk_timeframe: '5m',
      initial_capital: 5000,
      position_size: 0.4,
      stop_loss: 0.03,
      take_profit: 0.1,
      check_interval: 30,
    })
    expect((form.params.nested as { value: number }).value).toBe(1)
  })

  it('运行中状态优先使用当前运行参数并保留既有字段回退', () => {
    const current = controlForm({
      symbol: 'BTC-USDT-SWAP',
      timeframe: '15m',
      risk_timeframe: '1m',
      initial_capital: 1000,
      params: { old: true },
    })
    const running = status({
      running: true,
      strategy_id: 'runtime-a',
      symbol: '',
      timeframe: '1H',
      risk_timeframe: '5m',
      initial_capital: 0,
      position_size: 0,
      stop_loss: 0.02,
      take_profit: 0.08,
      check_interval: 15,
      params: { live: { enabled: true } },
    })

    const form = liveStrategyFormFromRunningStatus(running, current)
    ;(running.params.live as { enabled: boolean }).enabled = false

    expect(form).toMatchObject({
      strategy_id: 'runtime-a',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      risk_timeframe: '5m',
      initial_capital: current.initial_capital,
      position_size: current.position_size,
      stop_loss: 0.02,
      take_profit: 0.08,
      check_interval: 15,
    })
    expect((form?.params.live as { enabled: boolean }).enabled).toBe(true)
    expect(liveStrategyFormFromRunningStatus(status({ running: false }), current)).toBeNull()
  })

  it('应用表单状态时复制所有字段并隔离 params 引用', () => {
    const form = controlForm()
    const next = controlForm({
      strategy_id: 'next',
      symbol: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      params: { layer: { id: 'a' } },
    })

    applyLiveStrategyFormState(form, next)
    ;(next.params.layer as { id: string }).id = 'b'

    expect(form).toMatchObject({
      strategy_id: 'next',
      symbol: 'ETH-USDT-SWAP',
      timeframe: '1H',
    })
    expect((form.params.layer as { id: string }).id).toBe('a')
  })

  it('策略 runtime 选项和强制字段只来自对应策略', () => {
    const strategies = [
      strategyMeta({
        id: 'strategy-a',
        runtime: { ...runtimeConfig(), symbol: 'ETH-USDT-SWAP', timeframe: '1H' },
      }),
    ]
    const form = controlForm({ strategy_id: 'strategy-a', symbol: 'BTC-USDT-SWAP', inst_type: 'SPOT', timeframe: '15m' })

    expect(strategyRuntimeSymbolOptions(strategies, 'strategy-a', 'BTC-USDT-SWAP')).toEqual([
      { value: 'ETH-USDT-SWAP', label: 'ETH-USDT-SWAP' },
    ])
    expect(strategyRuntimeTimeframeOptions(strategies, 'missing', '15m')).toEqual([
      { value: '15m', label: '15m' },
    ])
    enforceStrategyRuntimeSymbol(form, strategies[0])
    enforceStrategyRuntimeInstType(form, strategies[0])
    enforceStrategyRuntimeTimeframe(form, strategies[0])

    expect(form.symbol).toBe('ETH-USDT-SWAP')
    expect(form.inst_type).toBe('SWAP')
    expect(form.timeframe).toBe('1H')
  })
})

function controlForm(overrides: Partial<LiveStrategyControlForm> = {}): LiveStrategyControlForm {
  return {
    ...DEFAULT_LIVE_CONTROL_FORM,
    params: {},
    ...overrides,
  }
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
    description: '',
    runtime: runtimeConfig(),
    visualization: {},
    decision_contract: {},
    ...overrides,
  }
}

function runtimeConfig(): NonNullable<StrategyMeta['runtime']> {
  return {
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    risk_timeframe: '1m',
    initial_capital: 1000,
    position_size: 0.25,
    stop_loss: 0,
    take_profit: 0,
    check_interval: 60,
    mode: 'simulated',
    params: {},
  }
}
