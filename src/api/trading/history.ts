import { apiGet, apiPost } from '../client'
import { arrayRecords } from '../normalize'
import {
  normalizeCostBasis,
  normalizeLocalFill,
  normalizeLocalFillsSyncResult,
  normalizePerformance,
} from './normalize'
import {
  isPresent,
  modeParams,
} from './shared'

export function fetchCostBasis(mode?: string) {
  return apiGet<unknown>('/api/trading/cost-basis', modeParams(mode))
    .then(data => arrayRecords(data).map(normalizeCostBasis).filter(isPresent))
}

export function fetchLocalFills(mode?: string) {
  return apiGet<unknown>('/api/trading/local-fills', modeParams(mode))
    .then(data => arrayRecords(data).map(normalizeLocalFill).filter(isPresent))
}

export function syncLocalFillsHistory(options: {
  mode?: string
  inst_type?: string
  inst_id?: string
  limit?: number
  after?: string
  before?: string
} = {}) {
  const body: Record<string, unknown> = {}
  if (options.mode) body.mode = options.mode
  if (options.inst_type) body.inst_type = options.inst_type
  if (options.inst_id) body.inst_id = options.inst_id
  if (typeof options.limit === 'number' && Number.isFinite(options.limit)) body.limit = options.limit
  if (options.after) body.after = options.after
  if (options.before) body.before = options.before
  return apiPost<unknown>('/api/trading/local-fills/sync', body).then(normalizeLocalFillsSyncResult)
}

export function fetchPerformance(mode?: string) {
  return apiGet<unknown>('/api/trading/performance', modeParams(mode))
    .then(data => arrayRecords(data).map(normalizePerformance).filter(isPresent))
}
