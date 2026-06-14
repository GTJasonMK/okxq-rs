import { apiGet, apiPost, apiDelete } from './client'
import {
  arrayRecords,
  recordFrom,
} from './normalize'
import {
  normalizeCondition,
  normalizeProfile,
  normalizeProfilePayload,
  normalizeResult,
  normalizeScanResponse,
} from './scanner/normalize'

export function fetchProfiles() {
  return apiGet<unknown>('/api/scanner/profiles').then(data => arrayRecords(data).map(normalizeProfile))
}

export function createProfile(data: Record<string, unknown>) {
  return apiPost<unknown>('/api/scanner/profiles', normalizeProfilePayload(data))
    .then(data => normalizeProfile(recordFrom(data)))
}

export function deleteProfile(profileId: string) {
  return apiDelete(`/api/scanner/profiles/${profileId}`)
}

export function runScan(data?: Record<string, unknown>) {
  return apiPost<unknown>('/api/scanner/scan', normalizeProfilePayload(data ?? {})).then(normalizeScanResponse)
}

export function runProfileScan(profileId: string) {
  return apiPost<unknown>(`/api/scanner/scan/${profileId}`).then(normalizeScanResponse)
}

export function fetchResults() {
  return apiGet<unknown>('/api/scanner/results').then(data => arrayRecords(data).map(normalizeResult))
}

export function fetchConditions() {
  return apiGet<unknown>('/api/scanner/conditions').then(data => arrayRecords(data).map(normalizeCondition))
}
