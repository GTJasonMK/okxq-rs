import { computed, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import { describe, expect, it } from 'vitest'
import { useLiveStrategyConfigSelection } from '@/composables/useLiveStrategyConfigSelection'
import type { LiveStrategyStatus, StrategyMeta } from '@/types'
import { status } from './fixtures/liveStrategy'

describe('useLiveStrategyConfigSelection', () => {
  it('reconciles strategy availability into form state and select options', () => {
    const selection = createSelection()

    selection.setStrategies([
      strategyMeta({
        id: 'strategy-a',
        name: 'Strategy A',
        runtime: runtime({ symbol: 'BTC-USDT-SWAP', timeframe: '15m' }),
      }),
      strategyMeta({
        id: 'strategy-b',
        name: 'Strategy B',
        runtime: runtime({ symbol: 'ETH-USDT-SWAP', timeframe: '1H' }),
      }),
    ])

    expect(selection.reconcileStrategyAvailability()).toBe(true)
    expect(selection.form.strategy_id).toBe('strategy-a')
    expect(selection.form.symbol).toBe('BTC-USDT-SWAP')
    expect(selection.strategyOptions.value).toContainEqual({ value: 'strategy-b', label: 'Strategy B' })

    expect(selection.setStrategyId('strategy-b')).toBe(true)

    expect(selection.form.strategy_id).toBe('strategy-b')
    expect(selection.form.symbol).toBe('ETH-USDT-SWAP')
    expect(selection.timeframeOptions.value).toEqual([{ value: '1H', label: '1H' }])
    expect(selection.symbolOptions.value).toEqual([{ value: 'ETH-USDT-SWAP', label: 'ETH-USDT-SWAP' }])
  })

  it('keeps unknown strategy ids explicit until backend strategy list changes', () => {
    const selection = createSelection()
    selection.form.params = { stale: true }

    expect(selection.setStrategyId('missing-strategy')).toBe(true)

    expect(selection.form.strategy_id).toBe('missing-strategy')
    expect(selection.form.params).toEqual({})
    expect(selection.strategyIds.value).toEqual([])
  })

  it('syncs running status and blocks edits while form is locked', () => {
    const currentStatus = ref(status({
      running: true,
      strategy_id: 'running-strategy',
      symbol: 'ETH-USDT-SWAP',
      timeframe: '1H',
      position_size: 0.4,
    }))
    const selection = createSelection({ status: currentStatus, locked: computed(() => true) })

    expect(selection.syncFormWithRunningStatus(currentStatus.value)).toBe(true)
    expect(selection.form.strategy_id).toBe('running-strategy')
    expect(selection.form.symbol).toBe('ETH-USDT-SWAP')
    expect(selection.form.position_size).toBe(0.4)
    expect(selection.setStrategyId('other')).toBe(false)
    expect(selection.applyStrategyRuntime('other')).toBe(false)
  })
})

function createSelection(options: {
  status?: Ref<LiveStrategyStatus | null>
  locked?: ComputedRef<boolean>
} = {}) {
  return useLiveStrategyConfigSelection({
    status: options.status ?? ref<LiveStrategyStatus | null>(null),
    formLocked: options.locked ?? computed(() => false),
  })
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
    description: '',
    runtime: runtime(),
    visualization: {},
    decision_contract: {},
    ...overrides,
  }
}

function runtime(overrides: Partial<NonNullable<StrategyMeta['runtime']>> = {}): NonNullable<StrategyMeta['runtime']> {
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
    ...overrides,
  }
}
