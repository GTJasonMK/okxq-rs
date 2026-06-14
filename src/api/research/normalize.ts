import {
  arrayRecords,
  arrayValue,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'

type UnknownRecord = Record<string, unknown>

export function normalizeDataset(raw: unknown) {
  const item = recordFrom(raw)
  const id = stringValue(item.dataset_id)
  return {
    ...item,
    id,
    name: id,
    dataset_id: id,
    inst_id: stringValue(item.inst_id),
    inst_type: stringValue(item.inst_type),
    timeframe: stringValue(item.timeframe, '1H'),
    status: stringValue(item.status),
    created_at: normalizeUnixSeconds(item.created_at),
    updated_at: normalizeUnixSeconds(item.updated_at),
  }
}

export function normalizeTrainingRun(raw: unknown) {
  const item = recordFrom(raw)
  const metrics = recordFrom(item.metrics)
  const validationMetrics = recordFrom(metrics.val)
  const testMetrics = recordFrom(metrics.test)
  const id = stringValue(item.run_id)
  return {
    ...item,
    id,
    run_id: id,
    dataset_id: stringValue(item.dataset_id),
    status: stringValue(item.status),
    progress_stage: stringValue(item.progress_stage),
    r2: numberValue(validationMetrics.r_squared),
    mse: numberValue(testMetrics.mse ?? validationMetrics.mse),
    mae: numberValue(testMetrics.mae ?? validationMetrics.mae),
    direction_accuracy: numberValue(validationMetrics.direction_accuracy),
    created_at: normalizeUnixSeconds(item.created_at),
    updated_at: normalizeUnixSeconds(item.updated_at),
  }
}

export function normalizeFactorValue(raw: unknown) {
  const item = recordFrom(raw)
  return {
    name: stringValue(item.factor_name),
    value: numberValue(item.value),
  }
}

export function normalizeTrendFactors(raw: unknown) {
  return arrayRecords(recordFrom(raw).rows).map(normalizeFactorValue)
}

export function normalizeTrendConfig(raw: unknown) {
  const item = recordFrom(raw)
  const whitelist = stringList(item.whitelist)
  return {
    symbol: whitelist[0] || '',
    inst_type: 'SWAP',
    timeframe: '1H',
    bar_count: 500,
    enabled: item.enabled === true,
    whitelist,
  }
}

export function normalizeTrendConfigPayload(data: Record<string, unknown>): Record<string, unknown> {
  let symbol = stringValue(data.symbol).trim().toUpperCase()
  const instType = normalizeInstType(data.inst_type)
  if (symbol && !symbol.includes('-')) symbol = `${symbol}-USDT`
  if (instType === 'SWAP' && symbol && !symbol.endsWith('-SWAP')) symbol = `${symbol}-SWAP`
  if (instType === 'SPOT' && symbol.endsWith('-SWAP')) symbol = symbol.slice(0, -5)

  const whitelist = stringList(data.whitelist)
  const body: Record<string, unknown> = {
    inst_type: instType,
  }
  if (typeof data.enabled === 'boolean') body.enabled = data.enabled
  if (symbol) body.whitelist = [symbol]
  else if (whitelist.length > 0) body.whitelist = whitelist
  putFiniteNumber(body, data, 'feature_bar_seconds')
  putFiniteNumber(body, data, 'state_sync_seconds')
  if (typeof data.book_channel === 'string') body.book_channel = data.book_channel
  return body
}

function stringList(value: unknown): string[] {
  return arrayValue(value).filter((item): item is string => typeof item === 'string').filter(Boolean)
}

function normalizeInstType(value: unknown): 'SPOT' | 'SWAP' {
  const instType = stringValue(value, 'SWAP').trim().toUpperCase()
  return instType === 'SPOT' ? 'SPOT' : 'SWAP'
}

function putFiniteNumber(target: UnknownRecord, source: UnknownRecord, key: string) {
  const value = source[key]
  if (typeof value === 'number' && Number.isFinite(value)) target[key] = value
}

function normalizeUnixSeconds(value: unknown): string {
  const seconds = numberValue(value, Number.NaN)
  if (!Number.isFinite(seconds) || seconds <= 0) return ''
  const milliseconds = seconds * 1000
  const date = new Date(milliseconds)
  return Number.isFinite(date.getTime()) ? date.toISOString() : ''
}
