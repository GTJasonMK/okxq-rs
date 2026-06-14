import { apiGet, apiPost, apiPut } from './client'
import {
  arrayRecords,
  recordFrom,
} from './normalize'
import {
  normalizeAssistantStatus,
  normalizeChatResponse,
  normalizeOrderDraft,
  normalizePatrolConfig,
  normalizePatrolConfigRequest,
  normalizePatrolConfigUpdate,
  normalizePatrolRun,
  normalizePatrolStatus,
  normalizeSession,
  normalizeSessionDetail,
  normalizeSessionRequest,
  normalizeToolCapability,
} from './assistant/normalize'

export function fetchAssistantStatus() {
  return apiGet<unknown>('/api/assistant/status').then(normalizeAssistantStatus)
}

export function fetchAgentCapabilities() {
  return apiGet<unknown>('/api/assistant/agent/tools')
    .then(data => arrayRecords(data).map(normalizeToolCapability))
}

export function fetchSessions() {
  return apiGet<unknown>('/api/assistant/agent/sessions')
    .then(data => arrayRecords(data).map(normalizeSession))
}

export function createSession(data: Record<string, unknown>) {
  return apiPost<unknown>('/api/assistant/agent/sessions', normalizeSessionRequest(data))
    .then(data => normalizeSession(recordFrom(data)))
}

export function fetchSession(sessionId: string) {
  return apiGet<unknown>(`/api/assistant/agent/sessions/${sessionId}`).then(normalizeSessionDetail)
}

export function chat(sessionId: string, message: string) {
  return apiPost<unknown>('/api/assistant/agent/chat', { session_id: sessionId, message })
    .then(normalizeChatResponse)
}

export function fetchPatrolStatus() {
  return apiGet<unknown>('/api/assistant/agent/patrol/status').then(normalizePatrolStatus)
}

export function fetchPatrolConfig() {
  return apiGet<unknown>('/api/assistant/agent/patrol/config').then(normalizePatrolConfig)
}

export function updatePatrolConfig(data: Record<string, unknown>) {
  return apiPut<unknown>('/api/assistant/agent/patrol/config', normalizePatrolConfigRequest(data))
    .then(normalizePatrolConfigUpdate)
}

export function runPatrolNow() {
  return apiPost<unknown>('/api/assistant/agent/patrol/run-now').then(normalizePatrolRun)
}

export function fetchOrderDrafts() {
  return apiGet<unknown>('/api/assistant/agent/order-drafts')
    .then(data => arrayRecords(data).map(normalizeOrderDraft))
}
