import type { Timeframe } from '@/types'
import {
  CHECK_INTERVAL_MAX,
  CHECK_INTERVAL_MIN,
  POSITION_SIZE_MAX,
  POSITION_SIZE_MIN,
  STOP_LOSS_MAX,
  STOP_LOSS_MIN,
  TAKE_PROFIT_MAX,
  TAKE_PROFIT_MIN,
} from '@/utils/liveStrategyControl'
import { firstNumberRangeError } from '@/utils/liveStrategyCore'

type LiveRuntimeConfigForm = {
  symbol: string
  timeframe: Timeframe | string
  risk_timeframe: Timeframe | string
  initial_capital: unknown
  position_size: unknown
  stop_loss: unknown
  take_profit: unknown
  check_interval: unknown
  params: Record<string, unknown>
}

type LiveRuntimeConfigValidationInput = {
  form: LiveRuntimeConfigForm
  supportedTimeframes: string[]
  supportedSymbols: string[]
}

export function liveRuntimeConfigDisabledReason(input: LiveRuntimeConfigValidationInput) {
  const form = input.form
  if (!String(form.symbol || '').trim()) return '请先选择品种'
  if (!String(form.timeframe || '').trim()) return '请先选择周期'
  if (input.supportedTimeframes.length > 0 && !input.supportedTimeframes.includes(form.timeframe)) {
    return `当前策略不支持周期 ${form.timeframe}`
  }
  if (input.supportedSymbols.length > 0 && !input.supportedSymbols.includes(form.symbol)) {
    return `当前策略不支持品种 ${form.symbol}`
  }
  if (Object.prototype.hasOwnProperty.call(form.params || {}, 'portfolio_layers')) {
    return '实时策略已删除 portfolio_layers 本地组合架构，请从策略参数中删除 portfolio_layers'
  }
  const numericError = firstNumberRangeError([
    {
      label: '初始资金',
      value: form.initial_capital,
      min: 1,
      max: Number.POSITIVE_INFINITY,
      unit: '',
    },
    {
      label: '仓位大小',
      value: form.position_size,
      min: POSITION_SIZE_MIN,
      max: POSITION_SIZE_MAX,
      unit: '%',
    },
    {
      label: '止损比例',
      value: form.stop_loss,
      min: STOP_LOSS_MIN,
      max: STOP_LOSS_MAX,
      unit: '%',
    },
    {
      label: '止盈比例',
      value: form.take_profit,
      min: TAKE_PROFIT_MIN,
      max: TAKE_PROFIT_MAX,
      unit: '%',
    },
    {
      label: '检查间隔',
      value: form.check_interval,
      min: CHECK_INTERVAL_MIN,
      max: CHECK_INTERVAL_MAX,
      unit: '秒',
    },
  ])
  if (numericError) return numericError
  return ''
}
