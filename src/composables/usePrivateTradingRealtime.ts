import { computed } from 'vue'
import * as realtimeApi from '@/api/marketRealtime'
import * as tradingApi from '@/api/trading'
import {
  useRealtimeSubscriptionBundle,
  type RealtimeSubscriptionSpec,
} from '@/composables/useRealtimeSubscriptionBundle'
import { useTradingStore } from '@/stores/tradingStore'
import type { Fill, Order, Position } from '@/types'
import { ensureTradingStoreMode } from '@/utils/tradingStoreMode'

type TradingMode = 'simulated' | 'live'

const MAX_WS_FILLS = 100

export function usePrivateTradingRealtime(mode: () => TradingMode) {
  const store = useTradingStore()
  ensureTradingStoreMode(store, mode())

  const { connected, error } = useRealtimeSubscriptionBundle({
    source: () => [mode()],
    beforeSetup(ids) {
      ensureTradingStoreMode(store, tradingModeFromSource(ids))
    },
    listeners: [
      {
        eventName: 'okxq-private-account',
        handlePayload: payload => { applyAccount(payload) },
      },
      {
        eventName: 'okxq-private-order',
        handlePayload: payload => { applyOrder(payload) },
      },
      {
        eventName: 'okxq-private-algo-order',
        handlePayload: payload => { applyAlgoOrder(payload) },
      },
      {
        eventName: 'okxq-private-fill',
        handlePayload: payload => { applyFill(payload) },
      },
      {
        eventName: 'okxq-private-position',
        handlePayload: payload => { applyPosition(payload) },
      },
    ],
    subscriptions: ids => privateSubscriptionSpecs(tradingModeFromSource(ids)),
  })
  const connectedMode = computed<TradingMode | ''>(() => connected.value ? mode() : '')

  function applyAccount(payload: Record<string, unknown>) {
    if (!sameMode(payload)) return
    const account = tradingApi.normalizePrivateAccountEvent(payload)
    if (account) store.account = account
  }

  function applyOrder(payload: Record<string, unknown>) {
    if (!sameMode(payload)) return
    const order = tradingApi.normalizeOrder(payload)
    if (!order.ord_id) return
    upsertPendingOrder(order)
  }

  function applyAlgoOrder(payload: Record<string, unknown>) {
    if (!sameMode(payload)) return
    const order = normalizeAlgoOrder(payload)
    if (!order) return
    upsertPendingOrder(order)
  }

  function applyFill(payload: Record<string, unknown>) {
    if (!sameMode(payload)) return
    const fill = tradingApi.normalizeFill(payload)
    if (!fill.fill_id) return
    if (!isValidFill(fill)) return
    store.fills = [fill, ...store.fills.filter(item => item.fill_id !== fill.fill_id)]
      .slice(0, MAX_WS_FILLS)
  }

  function applyPosition(payload: Record<string, unknown>) {
    if (!sameMode(payload)) return
    const position = tradingApi.normalizePosition(payload)
    if (!position.inst_id) return
    upsertPosition(position, payload)
  }

  function upsertPendingOrder(order: Order) {
    const next = store.orders.filter(item => item.ord_id !== order.ord_id)
    if (isPendingOrder(order)) next.unshift(order)
    store.orders = next
  }

  function upsertPosition(position: Position, payload: Record<string, unknown>) {
    const quantity = position.pos
    if (typeof quantity !== 'number' || !Number.isFinite(quantity)) return

    const key = positionKey(position)
    const netPositionEvent = isNetPositionEvent(payload)
    const next = store.positions.filter(item =>
      netPositionEvent ? item.inst_id !== position.inst_id : positionKey(item) !== key,
    )
    if (Math.abs(quantity) > 0) {
      next.unshift(position)
    }
    store.positions = next
  }

  function positionKey(position: Position) {
    return `${position.inst_id}:${position.pos_side}`
  }

  function isNetPositionEvent(payload: Record<string, unknown>) {
    const raw = isRecord(payload.raw) ? payload.raw : {}
    return String(raw.posSide ?? '').trim().toLowerCase() === 'net'
  }

  function isPendingOrder(order: Order) {
    if (order.state !== 'live' && order.state !== 'partially_filled') return false
    if (!isPositiveFinite(order.sz)) return false
    return order.ord_type === 'market' || isPositiveFinite(order.px)
  }

  function isValidFill(fill: Fill) {
    return isPositiveFinite(fill.fill_px) && isPositiveFinite(fill.fill_sz)
  }

  function isPositiveFinite(value: number | null) {
    return typeof value === 'number' && Number.isFinite(value) && value > 0
  }

  function sameMode(payload: Record<string, unknown>) {
    const payloadMode = normalizeMode(payload.mode)
    if (hasMode(payload) && !payloadMode) return false
    return !payloadMode || payloadMode === mode()
  }

  return {
    connected,
    connectedMode,
    error,
  }
}

function privateSubscriptionSpecs(mode: TradingMode): RealtimeSubscriptionSpec[] {
  return [
    {
      key: `private-account:${mode}`,
      subscribe: () => realtimeApi.subscribeAccount(mode),
      unsubscribe: () => realtimeApi.unsubscribeAccount(mode),
    },
    {
      key: `private-orders:${mode}`,
      subscribe: () => realtimeApi.subscribeOrders(mode),
      unsubscribe: () => realtimeApi.unsubscribeOrders(mode),
    },
    {
      key: `private-algo-orders:${mode}`,
      subscribe: () => realtimeApi.subscribeAlgoOrders(mode),
      unsubscribe: () => realtimeApi.unsubscribeAlgoOrders(mode),
    },
    {
      key: `private-fills:${mode}`,
      subscribe: () => realtimeApi.subscribeFills(mode),
      unsubscribe: () => realtimeApi.unsubscribeFills(mode),
    },
    {
      key: `private-positions:${mode}`,
      subscribe: () => realtimeApi.subscribePositions(mode),
      unsubscribe: () => realtimeApi.unsubscribePositions(mode),
    },
  ]
}

function normalizeAlgoOrder(payload: Record<string, unknown>): Order | null {
  const orderId = textValue(payload.algo_id)
    || textValue(payload.algoId)
    || textValue(payload.algo_cl_ord_id)
  const instId = textValue(payload.inst_id) || textValue(payload.instId)
  if (!orderId || !instId) return null
  const raw = isRecord(payload.raw) ? payload.raw : {}
  return {
    ord_id: orderId,
    inst_id: instId,
    side: orderSide(payload.side ?? raw.side),
    ord_type: 'market',
    sz: positiveNumber(payload.actual_sz ?? raw.sz ?? raw.actualSz),
    px: positiveNumber(payload.actual_px ?? raw.triggerPx ?? raw.slTriggerPx ?? raw.tpTriggerPx),
    state: textValue(payload.state ?? raw.state) || 'live',
    fill_sz: positiveNumber(payload.actual_sz ?? raw.actualSz),
    fill_px: positiveNumber(payload.actual_px ?? raw.actualPx),
    avg_px: positiveNumber(payload.actual_px ?? raw.actualPx),
    pnl: null,
    ctime: timestampNumber(payload.c_time ?? raw.cTime),
  }
}

function textValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function positiveNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) return value
  if (typeof value === 'string') {
    const parsed = Number(value.trim())
    if (Number.isFinite(parsed) && parsed > 0) return parsed
  }
  return null
}

function timestampNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) return value
  if (typeof value === 'string') {
    const parsed = Number(value.trim())
    if (Number.isFinite(parsed) && parsed > 0) return parsed
  }
  return null
}

function orderSide(value: unknown): Order['side'] {
  const side = textValue(value).toLowerCase()
  return side === 'sell' ? 'sell' : 'buy'
}

function tradingModeFromSource(ids: readonly string[]): TradingMode {
  return ids[0] === 'live' ? 'live' : 'simulated'
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function normalizeMode(value: unknown): TradingMode | '' {
  const mode = String(value || '').trim().toLowerCase()
  if (mode === 'live') return 'live'
  if (mode === 'simulated') return 'simulated'
  return ''
}

function hasMode(payload: Record<string, unknown>) {
  return Object.prototype.hasOwnProperty.call(payload, 'mode')
}
