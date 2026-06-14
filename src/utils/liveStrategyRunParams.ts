import type { StrategyMeta } from '@/types'
import {
  draftRowFromValue,
  draftRowsFromParams,
  omitEngineParams,
  type ParamDraftKind,
  type ParamDraftRow,
} from '@/utils/backtestResultCard'
import type { LiveStrategyControlForm } from '@/utils/liveStrategyForm'

const optionalRuntimeParamSpecs = [
  ['max_leverage', '最大杠杆', 'number'],
  ['risk_control_enabled', '启用风险控制', 'boolean'],
  ['max_single_loss_ratio', '单笔最大亏损比例', 'number'],
  ['max_symbol_exposure_pct', '单标的最大暴露比例', 'number'],
  ['max_same_direction_exposure_pct', '同向最大暴露比例', 'number'],
  ['max_daily_loss_ratio', '每日最大亏损比例', 'number'],
  ['max_order_value', '最大下单金额', 'number'],
  ['require_stop_loss', '要求保护止损', 'boolean'],
  ['max_slippage_bps', '最大滑点bps', 'number'],
  ['max_slippage_pct', '最大滑点比例', 'number'],
  ['max_slippage', '最大滑点比例', 'number'],
  ['live_fill_sync_symbol_limit', '成交同步标的数', 'number'],
] as const

export function liveRunStrategyDraftRows(form: LiveStrategyControlForm): ParamDraftRow[] {
  return draftRowsFromParamsWithoutEngineFields(form.params)
}

export function liveRunRuntimeDraftRows(
  form: LiveStrategyControlForm,
  strategy: StrategyMeta | null,
): ParamDraftRow[] {
  return [
    runtimeRow('initial_capital', '初始资金', form.initial_capital, 'number'),
    runtimeRow('position_size', '仓位大小', form.position_size, 'number'),
    runtimeRow('stop_loss', '止损比例', form.stop_loss, 'number'),
    runtimeRow('take_profit', '止盈比例', form.take_profit, 'number'),
    runtimeRow('check_interval', '检查间隔秒', form.check_interval, 'number'),
    runtimeRow('risk_timeframe', '风控K线周期', form.risk_timeframe, 'string'),
    ...contractExecutionRows(form, strategy),
    ...optionalRuntimeRows(form.params),
  ]
}

function contractExecutionRows(
  form: LiveStrategyControlForm,
  strategy: StrategyMeta | null,
): ParamDraftRow[] {
  if (!isContractRuntime(form, strategy)) return []
  return [
    runtimeRow('contract_mode', '合约模式', booleanParam(form.params.contract_mode, true), 'boolean'),
    runtimeRow('td_mode', '保证金模式', marginModeParam(form.params), 'string'),
    runtimeRow('leverage', '杠杆倍数', numberParam(form.params.leverage, 1), 'number'),
  ]
}

function optionalRuntimeRows(params: Record<string, unknown>): ParamDraftRow[] {
  return optionalRuntimeParamSpecs.flatMap(([key, label, kind]) => {
    if (!Object.prototype.hasOwnProperty.call(params, key)) return []
    return [runtimeRow(key, label, params[key], kind)]
  })
}

function runtimeRow(
  key: string,
  label: string,
  value: unknown,
  kind: ParamDraftKind,
): ParamDraftRow {
  return draftRowFromValue({ key, label, value, depth: 0 }, kind)
}

function draftRowsFromParamsWithoutEngineFields(params: Record<string, unknown>): ParamDraftRow[] {
  return draftRowsFromParams(omitEngineParams(params))
}

function isContractRuntime(form: LiveStrategyControlForm, strategy: StrategyMeta | null) {
  const runtimeInstType = strategy?.runtime?.inst_type
  if (typeof runtimeInstType === 'string' && runtimeInstType.trim()) {
    return isContractInstType(runtimeInstType)
  }
  if (isContractInstType(form.inst_type)) return true
  return form.symbol.trim().toUpperCase().endsWith('-SWAP')
}

function isContractInstType(value: unknown) {
  if (typeof value !== 'string') return false
  const normalized = value.trim().toUpperCase()
  return normalized === 'SWAP' || normalized === 'FUTURES'
}

function marginModeParam(params: Record<string, unknown>) {
  const value = stringParam(params.td_mode)
    || stringParam(params.mgn_mode)
    || stringParam(params.margin_mode)
  return value === 'isolated' ? 'isolated' : 'cross'
}

function stringParam(value: unknown) {
  return typeof value === 'string' ? value.trim().toLowerCase() : ''
}

function booleanParam(value: unknown, fallback: boolean) {
  return typeof value === 'boolean' ? value : fallback
}

function numberParam(value: unknown, fallback: number) {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string') {
    const parsed = Number(value.trim())
    if (Number.isFinite(parsed)) return parsed
  }
  return fallback
}
