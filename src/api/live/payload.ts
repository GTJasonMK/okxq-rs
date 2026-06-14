import {
  isRecord,
  stringValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeInstId,
  normalizeInstType,
  normalizeTimeframe,
} from '../marketNormalize'

export function normalizeLaunchPayload(raw: Record<string, unknown>): Record<string, unknown> {
  return normalizeStartPayload(raw)
}

export function normalizeDecisionDiagnosticsPayload(raw: Record<string, unknown>): Record<string, unknown> {
  const payload = normalizeStartPayload(raw)
  const item = isRecord(raw) ? raw : {}
  setInteger(payload, 'limit', item.limit, 3)
  if (typeof item.fresh === 'boolean') payload.fresh = item.fresh
  if (isRecord(item.latest_candle)) payload.latest_candle = normalizeDiagnosticCandle(item.latest_candle)
  return payload
}

export function normalizeLiveParams(params: Record<string, unknown>): Record<string, unknown> {
  return { ...params }
}

function normalizeStartPayload(raw: Record<string, unknown>): Record<string, unknown> {
  const item = isRecord(raw) ? raw : {}
  const payload: Record<string, unknown> = {}
  const rawSymbol = stringValue(item.symbol)
  const rawInstType = item.inst_type

  const strategyId = stringValue(item.strategy_id).trim()
  if (strategyId) payload.strategy_id = strategyId

  if (rawSymbol.trim()) {
    const instType = normalizeInstType(rawInstType, inferInstTypeFromId(rawSymbol))
    payload.symbol = normalizeInstId(rawSymbol, instType)
    payload.inst_type = instType
  } else if (typeof rawInstType === 'string') {
    payload.inst_type = normalizeInstType(rawInstType)
  }

  const timeframe = normalizeTimeframe(stringValue(item.timeframe))
  if (timeframe) payload.timeframe = timeframe

  const riskTimeframe = normalizeTimeframe(stringValue(item.risk_timeframe))
  if (riskTimeframe) payload.risk_timeframe = riskTimeframe

  setNumber(payload, 'initial_capital', item.initial_capital)
  setNumber(payload, 'position_size', item.position_size)
  setNumber(payload, 'stop_loss', item.stop_loss)
  setNumber(payload, 'take_profit', item.take_profit)
  setInteger(payload, 'check_interval', item.check_interval, 1)

  const mode = normalizeStrictLiveMode(item.mode)
  if (mode) payload.mode = mode

  if (Object.prototype.hasOwnProperty.call(item, 'params')) {
    if (!isRecord(item.params)) {
      throw new TypeError('实时策略参数 params 必须是 JSON 对象')
    }
    rejectRemovedPortfolioLayers(item.params)
    payload.params = normalizeLiveParams(item.params)
  }

  return payload
}

function rejectRemovedPortfolioLayers(params: Record<string, unknown>) {
  if (!Object.prototype.hasOwnProperty.call(params, 'portfolio_layers')) return
  throw new TypeError('实时策略已删除 portfolio_layers 本地组合架构，请从启动参数中删除 portfolio_layers')
}

function normalizeStrictLiveMode(value: unknown): 'live' | 'simulated' | '' {
  const mode = stringValue(value).trim().toLowerCase()
  if (!mode) return ''
  if (mode === 'live') return 'live'
  if (mode === 'simulated') return 'simulated'
  throw new TypeError(`实时策略运行模式只支持 live 或 simulated，收到 ${mode}`)
}

function normalizeDiagnosticCandle(raw: Record<string, unknown>): Record<string, unknown> {
  const instId = stringValue(raw.inst_id)
  const instType = normalizeInstType(raw.inst_type, inferInstTypeFromId(instId))
  const timeframe = normalizeTimeframe(stringValue(raw.timeframe))
  const candle: Record<string, unknown> = {
    inst_id: normalizeInstId(instId, instType),
    inst_type: instType,
  }
  if (timeframe) candle.timeframe = timeframe
  setPositiveInteger(candle, 'timestamp', raw.timestamp)
  setNumber(candle, 'open', raw.open)
  setNumber(candle, 'high', raw.high)
  setNumber(candle, 'low', raw.low)
  setNumber(candle, 'close', raw.close)
  setNumber(candle, 'volume', raw.volume)
  setNumber(candle, 'volume_ccy', raw.volume_ccy)
  setNumber(candle, 'volume_quote', raw.volume_quote)
  const confirm = stringValue(raw.confirm)
  if (confirm) candle.confirm = confirm
  return candle
}

function setNumber(target: Record<string, unknown>, key: string, value: unknown) {
  if (typeof value === 'number' && Number.isFinite(value)) target[key] = value
}

function setInteger(target: Record<string, unknown>, key: string, value: unknown, min: number) {
  if (typeof value === 'number' && Number.isFinite(value)) target[key] = Math.max(min, Math.round(value))
}

function setPositiveInteger(target: Record<string, unknown>, key: string, value: unknown) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) return
  target[key] = Math.round(value)
}
