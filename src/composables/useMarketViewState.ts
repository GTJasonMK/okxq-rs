import { onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMarketStore } from '@/stores/marketStore'
import { useRealtimeTicker } from '@/composables/useRealtimeTicker'
import { useRealtimeCandle } from '@/composables/useRealtimeCandle'
import { useMarketPreferences } from '@/composables/useMarketPreferences'
import { useMarketRepairState } from '@/composables/marketViewState/repair'
import { useMarketSelectionState } from '@/composables/marketViewState/selection'
import { useMarketSnapshotState } from '@/composables/marketViewState/snapshot'
import * as api from '@/api/market'
import type { Timeframe } from '@/types'
import { describeError } from '@/utils/logger'
import type {
  CandleRangeDays,
  MarketInstType,
  MarketSettings,
} from '@/types/marketView'
import {
  DEFAULT_DEPTH_ORDERBOOK_SIZE,
  candleLimitForRange,
  clampOrderbookSize,
  DEFAULT_CANDLE_RANGE_DAYS,
  clampCandleRangeDaysForTimeframe,
  normalizeCandleRangeDays,
  normalizeMarketType,
  normalizeTimeframe,
} from '@/utils/marketView'

export function useMarketViewState() {
  const store = useMarketStore()
  const route = useRoute()
  const router = useRouter()

  const activeMarketType = ref<MarketInstType>('SPOT')
  const requestedOrderbookSize = ref(DEFAULT_DEPTH_ORDERBOOK_SIZE)
  const candleRangeDays = ref<CandleRangeDays>(DEFAULT_CANDLE_RANGE_DAYS)

  let candleLoadSequence = 0

  const {
    activeBaseSymbol,
    activeWatchedSymbol,
    activeInstId,
    activeInstType,
    displayTicker,
    displayCandles,
    statusMessage,
    chartEmptyMessage,
    handleSymbolUpdate,
    setCandleRangeDays,
    alignActiveMarketType,
  } = useMarketSelectionState({
    activeMarketType,
    candleRangeDays,
    store,
  })

  useRealtimeTicker(() => {
    const ids = store.watchedSymbols
      .flatMap(item => [
        item.sync_spot ? item.spot_inst_id : '',
        item.sync_swap ? item.swap_inst_id : '',
      ])
      .filter(Boolean)
    return Array.from(new Set(ids))
  })

  useRealtimeCandle(() => activeInstId.value)

  const {
    displayOrderbook,
    depthOrderbook,
    displayTrades,
    loadMarketSnapshot,
    handleDepthRequest,
    resetMarketSnapshot,
  } = useMarketSnapshotState({
    activeInstId,
    activeInstType,
    requestedOrderbookSize,
    store,
  })

  function applyMarketSettings(settings: MarketSettings) {
    if (settings.activeSymbol) store.setActiveSymbol(settings.activeSymbol)
    if (settings.marketInstType) activeMarketType.value = settings.marketInstType
    if (settings.activeTimeframe) store.setActiveTimeframe(settings.activeTimeframe)
    if (settings.orderbookDepth) requestedOrderbookSize.value = clampOrderbookSize(settings.orderbookDepth)
    if (settings.candleRangeDays) {
      candleRangeDays.value = clampCandleRangeDaysForTimeframe(
        settings.candleRangeDays,
        store.activeTimeframe,
      )
    }
  }

  function currentMarketSettings(): MarketSettings {
    return {
      activeSymbol: activeBaseSymbol.value || store.activeSymbol,
      marketInstType: activeMarketType.value,
      activeTimeframe: store.activeTimeframe,
      orderbookDepth: requestedOrderbookSize.value,
      candleRangeDays: candleRangeDays.value,
    }
  }

  const {
    loadMarketPreferences,
    scheduleSaveMarketPreferences,
    flushMarketPreferences,
  } = useMarketPreferences({
    apply: applyMarketSettings,
    current: currentMarketSettings,
  })

  function applyRouteMarketOverrides() {
    const routeSymbol = api.normalizeBaseSymbol(String(route.query.symbol || ''))
    if (routeSymbol) store.setActiveSymbol(routeSymbol)
    const routeType = normalizeMarketType(route.query.inst_type ?? route.query.market_type ?? route.query.type)
    if (routeType) activeMarketType.value = routeType
    const routeTimeframe = normalizeTimeframe(route.query.timeframe ?? route.query.bar)
    if (routeTimeframe) store.setActiveTimeframe(routeTimeframe)
    const routeRangeDays = normalizeCandleRangeDays(
      route.query.range_days ?? route.query.candle_range_days ?? route.query.range ?? route.query.days
    )
    if (routeRangeDays) {
      candleRangeDays.value = clampCandleRangeDaysForTimeframe(routeRangeDays, store.activeTimeframe)
    }
  }

  async function loadSymbols() {
    try {
      store.watchedSymbols = await api.fetchWatchedSymbols()
      const routeSymbol = api.normalizeBaseSymbol(String(route.query.symbol || ''))
      applyRouteMarketOverrides()
      if (
        !routeSymbol &&
        store.watchedSymbols.length > 0 &&
        !store.watchedSymbols.some(item => item.symbol === activeBaseSymbol.value)
      ) {
        store.setActiveSymbol(store.watchedSymbols[0].symbol)
      }
      alignActiveMarketType()
    } catch (e) {
      store.error = `自选列表加载失败: ${describeError(e)}`
    }
  }

  async function loadCandles() {
    const instId = activeInstId.value
    const instType = activeInstType.value
    const timeframe = store.activeTimeframe
    const sequence = ++candleLoadSequence
    if (!instId) return
    try {
      store.loading = true
      store.error = null
      const candles = await api.fetchCandles(instId, {
        timeframe,
        inst_type: instType,
        limit: candleLimitForRange(timeframe, candleRangeDays.value),
      })
      if (
        sequence === candleLoadSequence &&
        instId === activeInstId.value &&
        timeframe === store.activeTimeframe
      ) {
        store.setCandles(instId, timeframe, candles)
      }
    } catch (e) {
      if (sequence === candleLoadSequence) store.error = describeError(e)
    } finally {
      if (sequence === candleLoadSequence) store.loading = false
    }
  }

  const {
    repairing,
    repairProgress,
    repairActive,
    openDataCenter,
  } = useMarketRepairState({
    activeBaseSymbol,
    activeWatchedSymbol,
    loadCandles,
    loadMarketSnapshot,
    router,
    store,
  })

  onMounted(async () => {
    await loadMarketPreferences()
    await loadSymbols()
    await loadCandles()
    await loadMarketSnapshot()
  })

  watch(
    () => [
      route.query.symbol,
      route.query.inst_type,
      route.query.market_type,
      route.query.type,
      route.query.timeframe,
      route.query.bar,
      route.query.range_days,
      route.query.candle_range_days,
      route.query.range,
      route.query.days,
    ],
    () => {
      applyRouteMarketOverrides()
      alignActiveMarketType()
    }
  )
  watch(
    activeWatchedSymbol,
    watched => {
      if (!watched) return
      alignActiveMarketType()
    },
    { immediate: true }
  )
  watch(
    () => store.activeTimeframe,
    timeframe => {
      const nextRange = clampCandleRangeDaysForTimeframe(candleRangeDays.value, timeframe)
      if (nextRange !== candleRangeDays.value) candleRangeDays.value = nextRange
    }
  )
  watch([activeInstId, () => store.activeTimeframe as Timeframe, candleRangeDays], () => {
    resetMarketSnapshot()
    loadCandles()
    loadMarketSnapshot()
  })
  watch(
    [activeBaseSymbol, activeMarketType, () => store.activeTimeframe, requestedOrderbookSize, candleRangeDays],
    () => scheduleSaveMarketPreferences()
  )
  onUnmounted(() => {
    flushMarketPreferences()
  })

  return {
    store,
    activeMarketType,
    repairing,
    repairProgress,
    candleRangeDays,
    activeWatchedSymbol,
    displayTicker,
    displayCandles,
    displayOrderbook,
    depthOrderbook,
    displayTrades,
    statusMessage,
    chartEmptyMessage,
    handleSymbolUpdate,
    setCandleRangeDays,
    handleDepthRequest,
    repairActive,
    openDataCenter,
  }
}
