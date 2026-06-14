import { flushPromises } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import type { StrategyMeta } from '@/types'
import type { LiveStrategyStatus } from '@/types/live-strategy'
import { useSystemStore } from '@/stores/systemStore'
import {
  fetchAvailableStrategiesMock,
  fetchLiveOrdersMock,
  fetchLiveStatusMock,
  mountLiveStrategyView,
  setupLiveStrategyViewHarness,
  type LiveStrategyTestPinia,
} from './helpers/liveStrategyViewHarness'
import { deferred, status } from './fixtures/liveStrategy'

describe('useLiveStrategyView loading', () => {
  let pinia!: LiveStrategyTestPinia
  setupLiveStrategyViewHarness((value) => {
    pinia = value
  })

  it('exposes strategy options before slow runtime status finishes', async () => {
    const slowStatus = deferred<LiveStrategyStatus>()
    fetchLiveStatusMock.mockReturnValueOnce(slowStatus.promise)
    fetchAvailableStrategiesMock.mockResolvedValueOnce([
      strategyMeta({
        id: 'fast_loaded_strategy',
        name: 'Fast Loaded Strategy',
      }),
    ])

    const { view } = mountLiveStrategyView(pinia)
    await flushPromises()

    expect(view.strategyOptions.value).toContainEqual({
      value: 'fast_loaded_strategy',
      label: 'Fast Loaded Strategy',
    })
    expect(view.status.value).toBeNull()
    expect(fetchLiveOrdersMock).not.toHaveBeenCalled()

    slowStatus.resolve(status({ running: false, status: 'stopped' }))
    await flushPromises()

    expect(view.form.strategy_id).toBe('fast_loaded_strategy')
    expect(fetchLiveOrdersMock).toHaveBeenCalled()
  })

  it('loads strategy options without waiting for slow system status', async () => {
    const slowSystemStatus = deferred<void>()
    const systemStore = useSystemStore()
    systemStore.statusLoaded = false
    const loadConfigSpy = vi
      .spyOn(systemStore, 'loadConfig')
      .mockReturnValueOnce(slowSystemStatus.promise)
    fetchAvailableStrategiesMock.mockResolvedValueOnce([
      strategyMeta({
        id: 'strategy_before_system_status',
        name: 'Strategy Before System Status',
      }),
    ])

    const { view } = mountLiveStrategyView(pinia)
    await flushPromises()

    expect(loadConfigSpy).toHaveBeenCalled()
    expect(fetchAvailableStrategiesMock).toHaveBeenCalled()
    expect(view.strategyOptions.value).toContainEqual({
      value: 'strategy_before_system_status',
      label: 'Strategy Before System Status',
    })

    slowSystemStatus.resolve()
    await flushPromises()
  })
})

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
    description: '',
    runtime: {
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
    },
    visualization: {},
    decision_contract: {},
    ...overrides,
  }
}
