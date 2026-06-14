import { defineStore } from 'pinia'
import { ref, computed, toRaw } from 'vue'
import type { Candle, Ticker, WatchedSymbol, Timeframe } from '@/types'
import { MAX_CHART_CANDLE_ROWS } from '@/utils/marketView'
import { isValidCandle } from '@/api/marketNormalize'
import { lowerBoundCandleTimestamp, sortedValidCandles } from '@/utils/marketView/candles/sorted'

export const useMarketStore = defineStore('market', () => {
  const watchedSymbols = ref<WatchedSymbol[]>([])
  const symbols = ref<string[]>([])
  const tickers = ref<Map<string, Ticker>>(new Map())
  const candles = ref<Map<string, Candle[]>>(new Map())
  const activeSymbol = ref('BTC-USDT')
  const activeTimeframe = ref<Timeframe>('1H')
  const loading = ref(false)
  const error = ref<string | null>(null)

  function cacheKey(instId: string, tf: string) { return `${instId}:${tf}` }
  function sortedCandles(data: Candle[]) {
    return sortedValidCandles(data).slice()
  }

  const activeTicker = computed(() => tickers.value.get(activeSymbol.value) ?? null)
  const activeCandles = computed(() =>
    candles.value.get(cacheKey(activeSymbol.value, activeTimeframe.value)) ?? []
  )

  function setActiveSymbol(s: string) { activeSymbol.value = s }
  function setActiveTimeframe(tf: Timeframe) { activeTimeframe.value = tf }

  function upsertTicker(t: Ticker) {
    tickers.value.set(t.inst_id, t)
    tickers.value = new Map(tickers.value) // trigger reactivity
  }

  function setCandles(instId: string, tf: Timeframe, data: Candle[]) {
    candles.value.set(cacheKey(instId, tf), sortedCandles(data))
    candles.value = new Map(candles.value)
  }

  function upsertCandle(candle: Candle, maxRows = MAX_CHART_CANDLE_ROWS) {
    if (!isValidCandle(candle)) return
    const key = cacheKey(candle.inst_id, candle.timeframe)
    const limit = Math.max(0, Math.floor(maxRows))
    if (limit === 0) {
      candles.value.set(key, [])
      candles.value = new Map(candles.value)
      return
    }
    const existing = candles.value.get(key) ?? []
    const rawExisting = toRaw(existing)
    const index = lowerBoundCandleTimestamp(rawExisting, candle.timestamp)
    if (index >= rawExisting.length && rawExisting.length < limit) {
      candles.value.set(key, [...rawExisting, candle])
      candles.value = new Map(candles.value)
      return
    }
    if (index >= rawExisting.length && rawExisting.length >= limit) {
      candles.value.set(key, [...rawExisting.slice(1), candle])
      candles.value = new Map(candles.value)
      return
    }
    if (index < rawExisting.length && rawExisting[index].timestamp === candle.timestamp) {
      const next = rawExisting.slice()
      next[index] = candle
      if (next.length > limit) next.splice(0, next.length - limit)
      candles.value.set(key, next)
      candles.value = new Map(candles.value)
      return
    }
    if (rawExisting.length >= limit && index === 0) {
      return
    }
    const next = rawExisting.slice()
    next.splice(index, 0, candle)
    if (next.length > limit) next.splice(0, next.length - limit)
    candles.value.set(key, next)
    candles.value = new Map(candles.value)
  }

  return {
    watchedSymbols, symbols, tickers, candles, activeSymbol, activeTimeframe, loading, error,
    activeTicker, activeCandles, setActiveSymbol, setActiveTimeframe, upsertTicker, setCandles, upsertCandle,
  }
})
