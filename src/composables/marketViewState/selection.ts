import { computed, type Ref } from 'vue'
import * as api from '@/api/market'
import type { InstType } from '@/types'
import type { useMarketStore } from '@/stores/marketStore'
import type { CandleRangeDays, MarketInstType } from '@/types/marketView'
import {
  aggregateSortedCandlesForRange,
  clampCandleRangeDaysForTimeframe,
  filterSortedCandlesByRange,
  mergeCandles,
} from '@/utils/marketView'

type MarketStore = ReturnType<typeof useMarketStore>

export function useMarketSelectionState(options: {
  activeMarketType: Ref<MarketInstType>
  candleRangeDays: Ref<CandleRangeDays>
  store: MarketStore
}) {
  const { activeMarketType, candleRangeDays, store } = options

  const activeBaseSymbol = computed(() => api.normalizeBaseSymbol(store.activeSymbol))
  const activeWatchedSymbol = computed(
    () => store.watchedSymbols.find(item => item.symbol === activeBaseSymbol.value) ?? null
  )
  const activeInstId = computed(() => {
    const watched = activeWatchedSymbol.value
    if (!watched) return ''
    if (activeMarketType.value === 'SWAP' && watched.sync_swap) return watched.swap_inst_id
    if (watched.sync_spot) return watched.spot_inst_id
    if (watched.sync_swap) return watched.swap_inst_id
    return ''
  })
  const activeInstType = computed<InstType>(() =>
    activeInstId.value.endsWith('-SWAP') ? 'SWAP' : 'SPOT'
  )
  const displayTicker = computed(() =>
    activeInstId.value ? (store.tickers.get(activeInstId.value) ?? null) : null
  )
  const displayCandles = computed(() => {
    if (!activeInstId.value) return []
    const timeframe = store.activeTimeframe
    const storedTarget = store.candles.get(`${activeInstId.value}:${timeframe}`) ?? []
    const baseCandles = store.candles.get(`${activeInstId.value}:1m`) ?? []
    if (timeframe === '1m') {
      return filterSortedCandlesByRange(
        baseCandles.length > 0 ? baseCandles : storedTarget,
        timeframe,
        candleRangeDays.value,
      )
    }
    const derived = aggregateSortedCandlesForRange(baseCandles, timeframe, candleRangeDays.value)
    const merged = derived.length > 0 ? mergeCandles(storedTarget, derived) : storedTarget
    return filterSortedCandlesByRange(
      merged,
      timeframe,
      candleRangeDays.value,
    )
  })
  const statusMessage = computed(() => {
    if (store.watchedSymbols.length === 0) {
      return '尚未添加关注币种。请先在数据中心添加关注并选择同步规则。'
    }
    if (!activeWatchedSymbol.value) {
      return `${activeBaseSymbol.value || store.activeSymbol} 未在关注清单中。行情页不会为未关注币种自动获取数据。`
    }
    return ''
  })
  const chartEmptyMessage = computed(() => {
    if (!activeWatchedSymbol.value) return '未关注的币种没有本地 K 线数据入口'
    if (store.loading) return ''
    if (displayCandles.value.length === 0) {
      return `${activeInstId.value} ${store.activeTimeframe} 尚未落库，请在数据中心或本页手动补齐`
    }
    return ''
  })

  function handleSymbolUpdate(value: string) {
    store.setActiveSymbol(api.normalizeBaseSymbol(value))
  }

  function setCandleRangeDays(value: CandleRangeDays) {
    candleRangeDays.value = clampCandleRangeDaysForTimeframe(value, store.activeTimeframe)
  }

  function alignActiveMarketType() {
    const watched = activeWatchedSymbol.value
    if (!watched) return
    if (activeMarketType.value === 'SPOT' && !watched.sync_spot && watched.sync_swap) {
      activeMarketType.value = 'SWAP'
    }
    if (activeMarketType.value === 'SWAP' && !watched.sync_swap && watched.sync_spot) {
      activeMarketType.value = 'SPOT'
    }
  }

  return {
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
  }
}
