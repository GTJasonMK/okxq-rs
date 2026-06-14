import { ref, watch } from 'vue'
import * as marketApi from '@/api/market'
import * as marketRealtimeApi from '@/api/marketRealtime'
import { inferInstTypeFromId, normalizeCandle } from '@/api/marketNormalize'
import { isRecord } from '@/api/normalize'
import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import { candleLimitForRange } from '@/utils/marketView'
import {
  mergeTriggerCandles,
  triggerCandleRequestMatches,
  type RealtimeTriggerCandle,
} from '@/utils/liveStrategyTrigger'
import { useRealtimeSubscriptionBundle } from './useRealtimeSubscriptionBundle'

type LiveTriggerValue<T> = { value: T }

type UseLiveTriggerCandlesInput = {
  selectedSymbol: LiveTriggerValue<string>
  timeframe: LiveTriggerValue<Timeframe>
  rangeDays: LiveTriggerValue<CandleRangeDays>
  onRealtimeCandle: (confirmed: boolean) => void
  onRealtimeError: (error: unknown) => void
}

export function useLiveTriggerCandles(input: UseLiveTriggerCandlesInput) {
  const triggerCandles = ref<Candle[]>([])
  const latestRealtimeTriggerCandle = ref<RealtimeTriggerCandle | null>(null)
  let triggerCandleLoadSequence = 0
  const { error: realtimeError } = useRealtimeSubscriptionBundle({
    source: triggerRealtimeSource,
    listeners: [{
      eventName: 'okxq-market-candle',
      handlePayload: payload => { applyRealtimeCandle(payload) },
    }],
    subscriptions: ([source]) => {
      const { instId, timeframe } = parseTriggerRealtimeSource(source)
      return [{
        key: `live-trigger-candle:${instId}:${timeframe}`,
        subscribe: () => marketRealtimeApi.subscribeCandle(instId, timeframe),
        unsubscribe: () => marketRealtimeApi.unsubscribeCandle(instId, timeframe),
      }]
    },
    resetOnEmptySource: resetRealtimeCandle,
    resetOnSourceChange: resetRealtimeCandle,
  })

  function triggerRealtimeSource() {
    const instId = input.selectedSymbol.value
    const timeframe = input.timeframe.value
    return instId && timeframe ? [triggerRealtimeSourceKey(instId, timeframe)] : []
  }

  async function loadTriggerCandles() {
    const sequence = ++triggerCandleLoadSequence
    const instId = input.selectedSymbol.value
    if (!instId) {
      triggerCandles.value = []
      return
    }
    const timeframe = input.timeframe.value
    const rangeDays = input.rangeDays.value
    let candles: Candle[]
    try {
      candles = await marketApi.fetchCandles(instId, {
        inst_type: inferInstTypeFromId(instId),
        timeframe,
        limit: candleLimitForRange(timeframe, rangeDays),
      })
    } catch (e) {
      if (isCurrentTriggerCandleRequest(sequence, instId, timeframe, rangeDays)) throw e
      return
    }
    if (!isCurrentTriggerCandleRequest(sequence, instId, timeframe, rangeDays)) return
    triggerCandles.value = candles
  }

  function resetRealtimeCandle() {
    latestRealtimeTriggerCandle.value = null
  }

  function applyRealtimeCandle(raw: unknown) {
    if (!isRecord(raw)) return
    const candle = normalizeCandle(raw)
    if (candle.inst_id !== input.selectedSymbol.value || candle.timeframe !== input.timeframe.value) {
      return
    }
    const confirm = normalizedConfirm(raw.confirm) ?? '0'
    latestRealtimeTriggerCandle.value = {
      ...candle,
      confirm,
    }
    upsertTriggerCandle(candle)
    input.onRealtimeCandle(confirm === '1')
  }

  function upsertTriggerCandle(candle: Candle) {
    const maxRows = Math.max(candleLimitForRange(input.timeframe.value, input.rangeDays.value), 300)
    triggerCandles.value = mergeTriggerCandles(triggerCandles.value, candle, maxRows)
  }

  function isCurrentTriggerCandleRequest(
    sequence: number,
    instId: string,
    timeframe: Timeframe,
    rangeDays: CandleRangeDays,
  ) {
    return triggerCandleRequestMatches(
      { sequence, instId, timeframe, rangeDays },
      {
        sequence: triggerCandleLoadSequence,
        instId: input.selectedSymbol.value,
        timeframe: input.timeframe.value,
        rangeDays: input.rangeDays.value,
      },
    )
  }

  watch(realtimeError, (error) => {
    if (error) input.onRealtimeError(error)
  })

  return {
    triggerCandles,
    latestRealtimeTriggerCandle,
    loadTriggerCandles,
  }
}

function triggerRealtimeSourceKey(instId: string, timeframe: Timeframe) {
  return `${instId}|${timeframe}`
}

function parseTriggerRealtimeSource(source: string) {
  const separatorIndex = source.lastIndexOf('|')
  if (separatorIndex <= 0) throw new Error('Invalid trigger realtime source')
  return {
    instId: source.slice(0, separatorIndex),
    timeframe: source.slice(separatorIndex + 1) as Timeframe,
  }
}

function normalizedConfirm(value: unknown): '0' | '1' | null {
  return value === '0' || value === '1' ? value : null
}
