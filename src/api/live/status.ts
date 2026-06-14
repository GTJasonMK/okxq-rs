import { apiGet, apiPost } from '../client'
import type * as T from '@/types/live-strategy'
import {
  isRecord,
  nullableNumberValue,
  nullableTimestampNumber,
  numberValue,
  stringValue,
} from '../normalize'
import { normalizeLaunchPayload, normalizeLiveParams } from './payload'
import { tradingMode } from './shared'

export function fetchLiveStatus() {
  return apiGet<unknown>('/api/live/status').then(normalizeStatus)
}

export function startLiveStrategy(data: Record<string, unknown>) {
  return apiPost<unknown>('/api/live/start', normalizeLaunchPayload(data)).then(normalizeStatus)
}

export function stopLiveStrategy() {
  return apiPost<unknown>('/api/live/stop').then(normalizeStatus)
}

function normalizeStatus(raw: unknown): T.LiveStrategyStatus {
  const item = isRecord(raw) ? raw : {}
  const status = stringValue(item.status, 'stopped')
  const normalizedStatus = status.toLowerCase()
  const running = ['starting', 'running', 'stopping'].includes(normalizedStatus)
  return {
    status,
    running,
    run_id: stringValue(item.run_id),
    strategy_id: stringValue(item.strategy_id),
    strategy_name: stringValue(item.strategy_name),
    symbol: stringValue(item.symbol),
    timeframe: stringValue(item.timeframe, '1H') as T.LiveStrategyStatus['timeframe'],
    inst_type: stringValue(item.inst_type, 'SPOT') as T.LiveStrategyStatus['inst_type'],
    initial_capital: numberValue(item.initial_capital),
    position_size: numberValue(item.position_size),
    stop_loss: numberValue(item.stop_loss),
    take_profit: numberValue(item.take_profit),
    params: isRecord(item.params) ? normalizeLiveParams(item.params) : {},
    risk_timeframe: stringValue(item.risk_timeframe, '1m') as T.LiveStrategyStatus['risk_timeframe'],
    mode: tradingMode(item.mode),
    start_time: stringValue(item.start_time) || null,
    last_action_time: stringValue(item.last_action_time) || null,
    last_action: stringValue(item.last_action),
    actions_generated: nullableNumberValue(item.total_actions),
    orders_placed: nullableNumberValue(item.total_orders),
    successful_orders: nullableNumberValue(item.successful_orders),
    failed_orders: nullableNumberValue(item.failed_orders),
    error_message: stringValue(item.error_message),
    check_interval: numberValue(item.check_interval, 60),
    execution_mode: stringValue(item.execution_mode, 'exchange_demo'),
    last_price: nullableNumberValue(item.last_price),
    last_action_strength: nullableNumberValue(item.last_action_strength),
    last_action_reason: stringValue(item.last_action_reason),
    last_order_candle_ts: nullableTimestampNumber(item.last_order_candle_ts),
  }
}
