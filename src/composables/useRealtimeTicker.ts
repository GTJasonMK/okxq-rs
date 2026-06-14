import * as api from '@/api/marketRealtime'
import { normalizeRealtimeTicker } from '@/api/marketRealtimeNormalize'
import { useMarketStore } from '@/stores/marketStore'
import { useRealtimeSubscriptionGroup } from './useRealtimeSubscriptionGroup'

export function useRealtimeTicker(symbols: () => string[]) {
  const store = useMarketStore()

  const { connected, error } = useRealtimeSubscriptionGroup({
    source: symbols,
    eventName: 'okxq-market-ticker',
    subscriptionKey: id => `ticker:${id}`,
    subscribe: api.subscribeTicker,
    unsubscribe: api.unsubscribeTicker,
    handlePayload(payload) {
      const ticker = normalizeRealtimeTicker(payload)
      if (ticker?.inst_id) {
        store.upsertTicker(ticker)
      }
    },
    clearErrorOnEmptySource: true,
  })

  return { connected, error }
}
