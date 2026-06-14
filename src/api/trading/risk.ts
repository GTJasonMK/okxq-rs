import { apiGet } from '../client'
import { normalizeRiskControl } from './normalize'

export function fetchRiskControl() {
  return apiGet<unknown>('/api/trading/risk-control').then(normalizeRiskControl)
}
