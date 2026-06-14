import * as api from '@/api/marketRealtime'
import { useMarketStore } from '@/stores/marketStore'
import type { Candle, Timeframe } from '@/types'
import { isRecord } from '@/api/normalize'
import { isValidCandle, normalizeCandle } from '@/api/marketNormalize'
import { MAX_CHART_CANDLE_ROWS } from '@/utils/marketView'
import { useSingleRealtimeSubscription } from './useSingleRealtimeSubscription'

const BASE_CANDLE_TIMEFRAME: Timeframe = '1m'

export function useRealtimeCandle(instId: () => string) {
  const store = useMarketStore()

  const { connected, error } = useSingleRealtimeSubscription({
    source: instId,
    eventName: 'okxq-market-candle',
    subscriptionKey: id => `candle:${id}:${BASE_CANDLE_TIMEFRAME}`,
    subscribe: id => api.subscribeCandle(id, BASE_CANDLE_TIMEFRAME),
    unsubscribe: id => api.unsubscribeCandle(id, BASE_CANDLE_TIMEFRAME),
    handlePayload(payload, currentInstId) {
      const candle = normalizeRealtimeCandle(payload)
      if (candle?.inst_id === currentInstId && candle.timeframe === BASE_CANDLE_TIMEFRAME) {
        store.upsertCandle(candle, MAX_CHART_CANDLE_ROWS)
      }
    },
  })

  return { connected, error }
}

function normalizeRealtimeCandle(raw: unknown): Candle | null {
  if (!isRecord(raw)) return null
  const candle = normalizeCandle(raw)
  if (!isValidCandle(candle)) return null
  return candle
}
