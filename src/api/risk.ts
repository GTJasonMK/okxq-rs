import { apiGet } from './client'
import { arrayRecords } from './normalize'
import {
  normalizeDrawdown,
  normalizeMetrics,
  normalizeOverview,
  normalizeRolling,
  normalizeSnapshot,
} from './risk/normalize'

export function fetchSnapshots() {
  return apiGet<unknown>('/api/risk/snapshots')
    .then(data => arrayRecords(data).map(normalizeSnapshot).filter(snapshot => snapshot !== null))
}

export function fetchMetrics() {
  return apiGet<unknown>('/api/risk/metrics').then(normalizeMetrics)
}

export function fetchDrawdown() {
  return apiGet<unknown>('/api/risk/drawdown').then(normalizeDrawdown)
}

export function fetchRolling() {
  return apiGet<unknown>('/api/risk/rolling').then(normalizeRolling)
}

export function fetchOverview() {
  return apiGet<unknown>('/api/risk/overview').then(normalizeOverview)
}
