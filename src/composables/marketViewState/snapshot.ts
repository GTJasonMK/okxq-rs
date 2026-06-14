import { computed, ref, watch, type ComputedRef, type Ref } from 'vue'
import * as api from '@/api/market'
import { useRealtimeOrderbook } from '@/composables/useRealtimeOrderbook'
import { useRealtimeTrades } from '@/composables/useRealtimeTrades'
import type { InstType, Orderbook, RecentTrade } from '@/types'
import type { useMarketStore } from '@/stores/marketStore'
import type { PendingOrderbookRequest } from '@/types/marketView'
import { describeError } from '@/utils/logger'
import { clampOrderbookSize, mergeDepthOrderbook } from '@/utils/marketView'

type MarketStore = ReturnType<typeof useMarketStore>

export function useMarketSnapshotState(options: {
  activeInstId: ComputedRef<string>
  activeInstType: ComputedRef<InstType>
  requestedOrderbookSize: Ref<number>
  store: MarketStore
}) {
  const {
    activeInstId,
    activeInstType,
    requestedOrderbookSize,
    store,
  } = options

  const snapshotOrderbook = ref<Orderbook | null>(null)
  const snapshotTrades = ref<RecentTrade[]>([])
  const snapshotOrderbookSize = ref(0)
  const pendingOrderbookRequest = ref<PendingOrderbookRequest | null>(null)
  let marketSnapshotSequence = 0
  let orderbookLoadSequence = 0

  const { orderbook: realtimeOrderbook, error: orderbookRealtimeError } = useRealtimeOrderbook(
    () => activeInstId.value
  )
  const { trades: realtimeTrades, error: tradesRealtimeError } = useRealtimeTrades(
    () => activeInstId.value
  )

  const displayOrderbook = computed(() => realtimeOrderbook.value ?? snapshotOrderbook.value)
  const depthOrderbook = computed(() =>
    mergeDepthOrderbook(realtimeOrderbook.value, snapshotOrderbook.value)
  )
  const displayTrades = computed(() =>
    realtimeTrades.value.length > 0 ? realtimeTrades.value : snapshotTrades.value
  )

  async function loadMarketSnapshot() {
    const symbol = activeInstId.value
    if (!symbol) {
      marketSnapshotSequence += 1
      orderbookLoadSequence += 1
      resetMarketSnapshot()
      return
    }
    const orderbookSize = requestedOrderbookSize.value
    const instType = activeInstType.value
    const snapshotSequence = ++marketSnapshotSequence
    const orderbookSequence = ++orderbookLoadSequence
    const [tickerResult, orderbookResult, tradesResult] = await Promise.allSettled([
      api.fetchTicker(symbol, instType),
      api.fetchOrderbook(symbol, orderbookSize, instType),
      api.fetchRecentTrades(symbol, 50, instType),
    ])
    if (
      snapshotSequence !== marketSnapshotSequence ||
      symbol !== activeInstId.value ||
      instType !== activeInstType.value
    ) {
      return
    }

    const errors: string[] = []
    if (tickerResult.status === 'fulfilled') {
      if (tickerResult.value) store.upsertTicker(tickerResult.value)
    }
    else errors.push(`Ticker: ${describeError(tickerResult.reason)}`)

    if (orderbookResult.status === 'fulfilled') {
      if (orderbookSequence === orderbookLoadSequence) {
        snapshotOrderbook.value = orderbookResult.value
        snapshotOrderbookSize.value = orderbookSize
      }
    } else {
      errors.push(`盘口: ${describeError(orderbookResult.reason)}`)
    }

    if (tradesResult.status === 'fulfilled') snapshotTrades.value = tradesResult.value
    else errors.push(`成交: ${describeError(tradesResult.reason)}`)

    if (errors.length > 0) store.error = errors.join('；')
  }

  async function handleDepthRequest(size: number) {
    const normalizedSize = clampOrderbookSize(size)
    requestedOrderbookSize.value = normalizedSize
    const symbol = activeInstId.value
    if (!symbol) return
    if (
      snapshotOrderbook.value?.inst_id === symbol &&
      snapshotOrderbookSize.value >= normalizedSize
    ) {
      return
    }
    const pending = pendingOrderbookRequest.value
    if (pending?.instId === symbol && pending.size >= normalizedSize) {
      return
    }

    const sequence = ++orderbookLoadSequence
    pendingOrderbookRequest.value = { instId: symbol, size: normalizedSize }
    try {
      const orderbook = await api.fetchOrderbook(symbol, normalizedSize, activeInstType.value)
      if (sequence !== orderbookLoadSequence || symbol !== activeInstId.value) return
      snapshotOrderbook.value = orderbook
      snapshotOrderbookSize.value = normalizedSize
    } catch (e) {
      if (sequence === orderbookLoadSequence) store.error = `盘口: ${describeError(e)}`
    } finally {
      const pendingAfter = pendingOrderbookRequest.value
      if (pendingAfter?.instId === symbol && pendingAfter.size === normalizedSize) {
        pendingOrderbookRequest.value = null
      }
    }
  }

  function resetMarketSnapshot() {
    snapshotOrderbook.value = null
    snapshotOrderbookSize.value = 0
    pendingOrderbookRequest.value = null
    snapshotTrades.value = []
  }

  watch([orderbookRealtimeError, tradesRealtimeError], ([orderbookError, tradesError]) => {
    const errors = [orderbookError, tradesError].filter(Boolean)
    if (errors.length > 0) store.error = errors.join('；')
  })

  return {
    displayOrderbook,
    depthOrderbook,
    displayTrades,
    loadMarketSnapshot,
    handleDepthRequest,
    resetMarketSnapshot,
  }
}
