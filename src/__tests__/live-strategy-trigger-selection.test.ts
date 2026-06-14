import { computed, nextTick, reactive, ref } from 'vue'
import type { ComputedRef } from 'vue'
import { describe, expect, it } from 'vitest'
import { useLiveStrategyTriggerSelection } from '@/composables/useLiveStrategyTriggerSelection'
import type { LiveOrder, LiveStrategyStatus, Timeframe } from '@/types'
import { liveOrder, status } from './fixtures/liveStrategy'

describe('useLiveStrategyTriggerSelection', () => {
  it('keeps selected trigger symbol and timeframe inside available options', async () => {
    const symbols = ref(['BTC-USDT-SWAP', 'ETH-USDT-SWAP'])
    const { selection } = createSelection({
      orders: [],
      symbolOptions: computed(() => symbols.value.map(value => ({ value, label: value }))),
    })

    expect(selection.triggerTimeframe.value).toBe('15m')
    selection.setTriggerSymbol('ETH-USDT-SWAP')
    selection.setTriggerTimeframe('1H')

    expect(selection.selectedTriggerSymbol.value).toBe('ETH-USDT-SWAP')
    expect(selection.selectedTriggerTimeframe.value).toBe('1H')
    expect(selection.triggerTimeframe.value).toBe('1H')

    symbols.value = ['BTC-USDT-SWAP']
    await nextTick()

    expect(selection.selectedTriggerSymbol.value).toBe('BTC-USDT-SWAP')
  })

  it('syncs single-strategy running status directly', () => {
    const running = status({
      running: true,
      symbol: 'ETH-USDT-SWAP',
      timeframe: '1H',
    })
    const { selection } = createSelection({ status: running })

    selection.syncTriggerSelectionForRunningStatus(running)

    expect(selection.selectedTriggerSymbol.value).toBe('ETH-USDT-SWAP')
    expect(selection.selectedTriggerTimeframe.value).toBe('1H')
  })
})

function createSelection(options: {
  status?: LiveStrategyStatus | null
  orders?: LiveOrder[]
  symbolOptions?: ComputedRef<{ value: string; label: string }[]>
  timeframes?: Timeframe[]
} = {}) {
  const currentStatus = ref<LiveStrategyStatus | null>(options.status ?? null)
  const form = reactive({
    symbol: 'BTC-USDT-SWAP',
    timeframe: '15m' as Timeframe,
  })
  const orders = ref(options.orders ?? [liveOrder({ inst_id: 'ETH-USDT-SWAP', symbol: 'ETH-USDT-SWAP' })])
  const symbolOptions = options.symbolOptions ?? computed(() => [
    { value: 'BTC-USDT-SWAP', label: 'BTC-USDT-SWAP' },
    { value: 'ETH-USDT-SWAP', label: 'ETH-USDT-SWAP' },
  ])
  const timeframes = options.timeframes ?? ['15m', '1H']
  const selection = useLiveStrategyTriggerSelection({
    status: currentStatus,
    form,
    activeStrategyId: computed(() => 'strategy'),
    symbolOptions,
    orders,
    strategyTimeframeOptions: () => timeframes.map(value => ({ value, label: value })),
  })
  return {
    currentStatus,
    form,
    orders,
    selection,
  }
}
