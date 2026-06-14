import { computed, defineComponent, h, reactive, ref } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as liveApi from '@/api/live'
import { useLiveStrategyDiagnostics } from '@/composables/useLiveStrategyDiagnostics'
import type { Timeframe } from '@/types'
import type { RealtimeTriggerCandle } from '@/utils/liveStrategyTrigger'
import { decisionDiagnostics, status } from './fixtures/liveStrategy'

vi.mock('@/api/live', () => ({
  fetchDecisionDiagnostics: vi.fn(),
}))

const fetchDecisionDiagnosticsMock = vi.mocked(liveApi.fetchDecisionDiagnostics)

describe('useLiveStrategyDiagnostics', () => {
  beforeEach(() => {
    fetchDecisionDiagnosticsMock.mockResolvedValue(decisionDiagnostics())
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('loads diagnostics with the current target and latest realtime candle', async () => {
    const { diagnostics, wrapper } = mountDiagnostics()

    await diagnostics.loadDecisionDiagnostics()

    expect(fetchDecisionDiagnosticsMock).toHaveBeenCalledWith(expect.objectContaining({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      timeframe: '15m',
      latest_candle: expect.objectContaining({
        inst_id: 'BTC-USDT-SWAP',
        timeframe: '15m',
        volume_ccy: 1200,
        volume_quote: 1200,
        confirm: '1',
      }),
    }))
    expect(diagnostics.currentDecisionDiagnostics.value?.strategy_id).toBe('multi_timeframe_dual_v12')
    expect(diagnostics.autoDecisionDiagnosticsEnabled.value).toBe(true)

    diagnostics.scheduleDecisionDiagnosticsRefresh(false)
    await flushPromises()
    expect(fetchDecisionDiagnosticsMock).toHaveBeenCalledTimes(1)

    diagnostics.scheduleDecisionDiagnosticsRefresh(true)
    await flushPromises()
    expect(fetchDecisionDiagnosticsMock).toHaveBeenCalledTimes(2)

    wrapper.unmount()
  })

  it('disarms automatic diagnostics when the target changes', async () => {
    const { form, diagnostics, wrapper } = mountDiagnostics()

    await diagnostics.loadDecisionDiagnostics()
    expect(diagnostics.autoDecisionDiagnosticsEnabled.value).toBe(true)

    form.symbol = 'ETH-USDT-SWAP'
    await flushPromises()

    expect(diagnostics.autoDecisionDiagnosticsEnabled.value).toBe(false)
    expect(diagnostics.currentDecisionDiagnostics.value).toBeNull()
    expect(diagnostics.decisionDiagnosticsError.value).toBeNull()

    wrapper.unmount()
  })

  it('coalesces repeated automatic diagnostics refreshes while one request is in flight', async () => {
    const { diagnostics, wrapper } = mountDiagnostics()

    await diagnostics.loadDecisionDiagnostics()
    fetchDecisionDiagnosticsMock.mockClear()

    const pending = deferred<ReturnType<typeof decisionDiagnostics>>()
    fetchDecisionDiagnosticsMock.mockReturnValueOnce(pending.promise)

    diagnostics.scheduleDecisionDiagnosticsRefresh(true)
    diagnostics.scheduleDecisionDiagnosticsRefresh(true)
    await flushPromises()

    expect(fetchDecisionDiagnosticsMock).toHaveBeenCalledTimes(1)

    pending.resolve(decisionDiagnostics())
    await flushPromises()

    expect(fetchDecisionDiagnosticsMock).toHaveBeenCalledTimes(1)

    wrapper.unmount()
  })
})

function mountDiagnostics() {
  const currentStatus = ref(status({ running: true, status: 'running' }))
  const form = reactive({
    strategy_id: 'multi_timeframe_dual_v12',
    symbol: 'BTC-USDT-SWAP',
    timeframe: '15m' as Timeframe,
    initial_capital: 1000,
    position_size: 0.25,
    stop_loss: 0,
    take_profit: 0,
    params: {},
  })
  const triggerTimeframe = ref<Timeframe>('15m')
  const latestRealtimeTriggerCandle = ref<RealtimeTriggerCandle | null>({
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
  })
  const error = ref<string | null>(null)
  let diagnostics!: ReturnType<typeof useLiveStrategyDiagnostics>
  const wrapper = mount(defineComponent({
    setup() {
      diagnostics = useLiveStrategyDiagnostics({
        status: currentStatus,
        form,
        controlMode: computed(() => 'simulated'),
        selectedTriggerSymbol: computed(() => form.symbol),
        triggerTimeframe: computed(() => triggerTimeframe.value),
        latestRealtimeTriggerCandle,
        defaultInitialCapital: 1000,
        error,
      })
      return () => h('div')
    },
  }))
  return {
    currentStatus,
    diagnostics,
    error,
    form,
    latestRealtimeTriggerCandle,
    triggerTimeframe,
    wrapper,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
