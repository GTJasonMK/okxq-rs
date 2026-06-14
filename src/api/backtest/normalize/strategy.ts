import type * as T from '@/types/backtest'
import {
  recordFrom,
  stringValue,
} from '../../normalize'
import type { AnyRecord } from './types'
import { numberValue } from './numbers'

export function normalizeStrategy(raw: AnyRecord): T.StrategyMeta {
  return {
    id: stringValue(raw.id),
    name: stringValue(raw.name),
    description: stringValue(raw.description),
    strategy_type: stringValue(raw.strategy_type),
    data_requirements: recordFrom(raw.data_requirements),
    runtime: normalizeStrategyRuntime(recordFrom(raw.runtime)),
    visualization: recordFrom(raw.visualization),
    decision_contract: recordFrom(raw.decision_contract),
  }
}

function normalizeStrategyRuntime(raw: AnyRecord): T.StrategyRuntimeConfig {
  return {
    symbol: stringValue(raw.symbol),
    inst_type: stringValue(raw.inst_type, 'SWAP') as T.StrategyRuntimeConfig['inst_type'],
    timeframe: stringValue(raw.timeframe, '15m') as T.StrategyRuntimeConfig['timeframe'],
    risk_timeframe: stringValue(raw.risk_timeframe, '1m') as T.StrategyRuntimeConfig['risk_timeframe'],
    initial_capital: numberValue(raw.initial_capital, 1000),
    position_size: numberValue(raw.position_size, 0.2),
    stop_loss: numberValue(raw.stop_loss),
    take_profit: numberValue(raw.take_profit),
    check_interval: numberValue(raw.check_interval, 60),
    mode: stringValue(raw.mode),
    params: recordFrom(raw.params),
  }
}
