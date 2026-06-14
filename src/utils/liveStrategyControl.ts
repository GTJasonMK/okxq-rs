import type { InstType, Timeframe } from '@/types'

export const DEFAULT_LIVE_CONTROL_FORM = {
  strategy_id: '',
  symbol: 'BTC-USDT-SWAP',
  inst_type: 'SWAP' as InstType,
  timeframe: '15m' as Timeframe,
  risk_timeframe: '1m' as Timeframe,
  initial_capital: 1000,
  position_size: 0.35,
  stop_loss: 0,
  take_profit: 0,
  check_interval: 60,
  params: {} as Record<string, unknown>,
}

export const LIVE_RUNTIME_REFRESH_INTERVAL_MS = 5_000
export const LIVE_EQUITY_REFRESH_INTERVAL_MS = 1_000
export const POSITION_SIZE_MIN = 0.01
export const POSITION_SIZE_MAX = 1.0
export const STOP_LOSS_MIN = 0
export const STOP_LOSS_MAX = 1.0
export const TAKE_PROFIT_MIN = 0
export const TAKE_PROFIT_MAX = 5.0
export const CHECK_INTERVAL_MIN = 1
export const CHECK_INTERVAL_MAX = 86_400
