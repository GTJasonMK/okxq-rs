import type {
  AnyRecord,
  EngineParamSpec,
  EngineParamSpecSource,
} from './types'
import { readableParamRows } from './readable'

const engineParamKeys = [
  'contract_mode',
  'fee_rate',
  'commission_rate',
  'slippage_rate',
  'funding_rate_8h',
  'leverage',
  'td_mode',
  'mgn_mode',
  'margin_mode',
  'position_size',
  'position_size_mode',
  'position_sizing',
  'position_size_is_notional',
  'position_size_is_effective_notional',
  'allow_short',
  'max_hold_bars',
  'maintenance_margin_rate',
  'execution_timing',
  'execution_model',
  'execution_delay_bars',
  'execution_price',
  'stop_loss',
  'take_profit',
  'max_slippage',
  'max_slippage_pct',
  'max_slippage_bps',
  '_runtime_max_slippage',
  'max_single_loss_ratio',
  'max_symbol_exposure_pct',
  'max_order_value',
  'require_stop_loss',
  'require_protective_stop',
  'require_protective_risk_order',
  'stop_loss_required',
  'risk_control_enabled',
  'max_leverage',
  'max_allowed_leverage',
  'max_daily_loss_ratio',
  'daily_loss_limit_ratio',
  'daily_loss_limit',
  'max_daily_loss_bps',
  'max_same_direction_exposure_pct',
  'max_correlated_exposure_pct',
  'max_same_side_exposure_pct',
  'backtest_instrument_rules_source',
  'instrument_rules_source',
  'historical_instrument_rules_source',
  'ctVal',
  'ctValCcy',
  'ct_val',
  'ct_val_ccy',
  'contract_value',
  'contract_value_ccy',
  'lotSz',
  'minSz',
  'tickSz',
  'lot_size',
  'min_size',
  'tick_size',
  'live_fill_sync_symbol_limit',
  'fill_sync_symbol_limit',
] as const

const engineParamKeySet = new Set<string>(engineParamKeys)

export function pickEngineParams(params: AnyRecord) {
  const picked: Record<string, unknown> = {}
  for (const key of engineParamKeys) {
    if (Object.prototype.hasOwnProperty.call(params, key)) {
      picked[key] = params[key]
    }
  }
  return picked
}

function isEngineParamKey(key: string) {
  return engineParamKeySet.has(key)
}

export function omitEngineParams(params: AnyRecord) {
  const output: AnyRecord = {}
  for (const [key, value] of Object.entries(params)) {
    if (!isEngineParamKey(key)) {
      output[key] = value
    }
  }
  return output
}

export function engineParamSpecsFromSource(source: EngineParamSpecSource): EngineParamSpec[] {
  const costModel = source.costModel ?? {}
  const executionModel = source.executionModel ?? {}
  const params = source.params ?? {}
  const runtime = source.runtime ?? {}
  const contractMode = firstDefined(costModel.contract_mode, source.contractMode, params.contract_mode, runtimeContractMode(runtime.inst_type))
  const simulatedContractMode = contractMode === true
  const simulatedCcy = symbolBaseCcy(runtime.symbol)
  const instrumentRulesSource = normalizedInstrumentRulesSource(firstDefined(
    params.backtest_instrument_rules_source,
    params.instrument_rules_source,
    params.historical_instrument_rules_source,
  ))
  return [
    { key: 'contract_mode', label: '合约模式', kind: 'boolean', value: contractMode },
    {
      key: 'backtest_instrument_rules_source',
      label: '交易规格来源',
      kind: 'select',
      value: instrumentRulesSource,
      options: [
        { label: '模拟规格', value: 'simulated' },
        { label: '手动参数', value: 'params' },
        { label: 'OKX 实时规格', value: 'okx' },
      ],
    },
    { key: 'ctVal', label: '合约面值 ctVal', kind: 'number', value: firstDefined(params.ctVal, params.ct_val, params.contract_value, simulatedContractMode ? 1 : undefined) },
    { key: 'ctValCcy', label: '合约面值币种', kind: 'string', value: firstDefined(params.ctValCcy, params.ct_val_ccy, params.contract_value_ccy, simulatedContractMode ? simulatedCcy : undefined) },
    { key: 'lotSz', label: '最小数量步长 lotSz', kind: 'number', value: firstDefined(params.lotSz, params.lot_size, simulatedContractMode ? 1 : 0.00000001) },
    { key: 'minSz', label: '最小下单数量 minSz', kind: 'number', value: firstDefined(params.minSz, params.min_size, simulatedContractMode ? 1 : 0.00001) },
    { key: 'tickSz', label: '价格步长 tickSz', kind: 'number', value: firstDefined(params.tickSz, params.tick_size, 0.00000001) },
    { key: 'fee_rate', label: '手续费率', kind: 'number', value: firstDefined(costModel.fee_rate, params.fee_rate, params.commission_rate) },
    { key: 'slippage_rate', label: '滑点率', kind: 'number', value: firstDefined(costModel.slippage_rate, params.slippage_rate) },
    { key: 'funding_rate_8h', label: '资金费率/8小时', kind: 'number', value: firstDefined(costModel.funding_rate_8h, params.funding_rate_8h) },
    { key: 'leverage', label: '杠杆倍数', kind: 'number', value: firstDefined(costModel.leverage, params.leverage) },
    { key: 'td_mode', label: '保证金模式', kind: 'string', value: firstDefined(params.td_mode, params.mgn_mode, params.margin_mode) },
    { key: 'position_size', label: '仓位大小', kind: 'number', value: firstDefined(costModel.position_size, params.position_size, runtime.position_size) },
    { key: 'position_size_mode', label: '仓位大小模式', kind: 'string', value: firstDefined(costModel.position_size_mode, params.position_size_mode, params.position_sizing) },
    { key: 'allow_short', label: '允许做空', kind: 'boolean', value: firstDefined(costModel.allow_short, params.allow_short) },
    { key: 'max_hold_bars', label: '最大持仓K线', kind: 'number', value: firstDefined(costModel.max_hold_bars, params.max_hold_bars) },
    { key: 'maintenance_margin_rate', label: '维持保证金率', kind: 'number', value: firstDefined(costModel.maintenance_margin_rate, params.maintenance_margin_rate) },
    { key: 'execution_timing', label: '执行时机', kind: 'string', value: firstDefined(executionModel.timing, params.execution_timing, params.execution_model) },
    { key: 'execution_delay_bars', label: '执行延迟K线数', kind: 'number', value: firstDefined(executionModel.delay_bars, params.execution_delay_bars) },
    { key: 'execution_price', label: '成交价格来源', kind: 'string', value: firstDefined(executionModel.price, params.execution_price) },
    { key: 'stop_loss', label: '止损比例', kind: 'number', value: firstDefined(params.stop_loss, runtime.stop_loss) },
    { key: 'take_profit', label: '止盈比例', kind: 'number', value: firstDefined(params.take_profit, runtime.take_profit) },
    { key: 'risk_control_enabled', label: '启用风险控制', kind: 'boolean', value: params.risk_control_enabled },
    { key: 'max_single_loss_ratio', label: '单笔最大亏损比例', kind: 'number', value: params.max_single_loss_ratio },
    { key: 'max_symbol_exposure_pct', label: '单标的最大暴露比例', kind: 'number', value: params.max_symbol_exposure_pct },
    { key: 'max_order_value', label: '最大下单金额', kind: 'number', value: params.max_order_value },
    { key: 'require_stop_loss', label: '要求止损', kind: 'boolean', value: params.require_stop_loss },
    { key: 'max_leverage', label: '最大杠杆', kind: 'number', value: params.max_leverage },
    { key: 'max_daily_loss_ratio', label: '每日最大亏损比例', kind: 'number', value: params.max_daily_loss_ratio },
    { key: 'max_same_direction_exposure_pct', label: '同向最大暴露比例', kind: 'number', value: params.max_same_direction_exposure_pct },
  ]
}

export function readableRowsFromEngineSpecs(specs: EngineParamSpec[]) {
  const payload: AnyRecord = {}
  for (const spec of specs) {
    payload[spec.key] = spec.value
  }
  return readableParamRows(payload)
}

function runtimeContractMode(instType: unknown) {
  if (typeof instType !== 'string' || !instType.trim()) return undefined
  const normalized = instType.toUpperCase()
  return normalized === 'SWAP' || normalized === 'FUTURES'
}

function symbolBaseCcy(symbol: unknown) {
  if (typeof symbol !== 'string') return 'BASE'
  return symbol.trim().split('-').filter(Boolean)[0]?.toUpperCase() || 'BASE'
}

function normalizedInstrumentRulesSource(value: unknown) {
  if (typeof value !== 'string') return 'simulated'
  const raw = value.trim()
  const normalized = raw.toLowerCase()
  if (!normalized) return 'simulated'
  if (['simulated', 'simulation', 'mock', 'local'].includes(normalized)) return 'simulated'
  if (['params', 'manual', 'custom'].includes(normalized)) return 'params'
  if (['okx', 'exchange', 'public', 'live'].includes(normalized)) return 'okx'
  return raw
}

function firstDefined(...values: unknown[]) {
  return values.find(value => value !== undefined && value !== null)
}
