import { ref } from 'vue'
import * as api from '@/api/marketRealtime'
import { normalizeRealtimeOrderbook } from '@/api/marketRealtimeNormalize'
import type { Orderbook } from '@/types'
import { useSingleRealtimeSubscription } from './useSingleRealtimeSubscription'

export function useRealtimeOrderbook(instId: () => string) {
  const orderbook = ref<Orderbook | null>(null)

  const { connected, error } = useSingleRealtimeSubscription({
    source: instId,
    eventName: 'okxq-market-orderbook',
    subscriptionKey: id => `orderbook:${id}`,
    subscribe: api.subscribeOrderbook,
    unsubscribe: api.unsubscribeOrderbook,
    handlePayload(payload, currentInstId) {
      const nextOrderbook = normalizeRealtimeOrderbook(payload)
      if (nextOrderbook.inst_id === currentInstId) orderbook.value = nextOrderbook
    },
    resetOnEmptySource() {
      orderbook.value = null
    },
  })

  return { orderbook, connected, error }
}
