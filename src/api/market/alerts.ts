import { apiDelete, apiGet, apiPatch, apiPost } from '../client'
import {
  normalizePriceAlert,
} from '../marketNormalize'
import {
  normalizePriceAlertPayload,
  normalizePriceAlerts,
} from './normalize'

export function fetchAlerts() {
  return apiGet<unknown>('/api/market/alerts').then(normalizePriceAlerts)
}

export function createAlert(data: Record<string, unknown>) {
  return apiPost<unknown>('/api/market/alerts', normalizePriceAlertPayload(data)).then(normalizePriceAlert)
}

export function updateAlert(alertId: string, data: Record<string, unknown>) {
  return apiPatch<unknown>(
    `/api/market/alerts/${encodeURIComponent(alertId)}`,
    normalizePriceAlertPayload(data),
  ).then(normalizePriceAlert)
}

export function deleteAlert(alertId: string) {
  return apiDelete(`/api/market/alerts/${encodeURIComponent(alertId)}`)
}
