import { ref } from 'vue'
import * as api from '@/api/marketRealtime'
import { normalizeRealtimeTrade } from '@/api/marketRealtimeNormalize'
import type { RecentTrade } from '@/types'
import { useSingleRealtimeSubscription } from './useSingleRealtimeSubscription'

const MAX_TRADES = 100

export function useRealtimeTrades(instId: () => string) {
  const trades = ref<RecentTrade[]>([])

  const { connected, error } = useSingleRealtimeSubscription({
    source: instId,
    eventName: 'okxq-market-trade',
    subscriptionKey: id => `trades:${id}`,
    subscribe: api.subscribeTrades,
    unsubscribe: api.unsubscribeTrades,
    handlePayload(payload, currentInstId) {
      const trade = normalizeRealtimeTrade(payload)
      if (!trade || trade.inst_id !== currentInstId) return
      const nextTrades = [trade, ...trades.value]
      if (nextTrades.length > MAX_TRADES) nextTrades.length = MAX_TRADES
      trades.value = nextTrades
    },
    resetOnEmptySource() {
      trades.value = []
    },
    resetOnSourceChange() {
      trades.value = []
    },
  })

  return { trades, connected, error }
}
