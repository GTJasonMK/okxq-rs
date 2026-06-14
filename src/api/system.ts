import { invoke } from '@tauri-apps/api/core'
import { apiGet } from './client'
import type * as T from '@/types/system'
import { describeError, logger } from '@/utils/logger'
import {
  normalizeAssistantConfigRequest,
  normalizeAssistantConfigResponse,
  normalizeOkxConfigRequest,
  normalizeOkxConfigResponse,
  normalizeOkxTestResult,
} from './system/normalize'

export function fetchHealth() {
  return apiGet<unknown>('/health')
}
export function fetchSystemStatus() {
  return apiGet<T.SystemStatus>('/status')
}
export function fetchOkxConfig() {
  return commandInvoke<unknown>('get_okx_config').then(normalizeOkxConfigResponse)
}
export function saveOkxConfig(data: T.OkxConfigSaveRequest) {
  return commandInvoke<unknown>('save_okx_config', { req: normalizeOkxConfigRequest(data) })
}
export function testOkxConfig(data: T.OkxConfigSaveRequest) {
  return commandInvoke<unknown>('test_okx_connection', { req: normalizeOkxConfigRequest(data) })
    .then(normalizeOkxTestResult)
}
export function fetchAssistantConfig() {
  return commandInvoke<unknown>('get_assistant_config').then(normalizeAssistantConfigResponse)
}
export function saveAssistantConfig(data: T.AssistantConfigSaveRequest) {
  return commandInvoke<unknown>('save_assistant_config', { req: normalizeAssistantConfigRequest(data) })
}
export function fetchPreference<T = unknown>(key: string) {
  return commandInvoke<T | null>('get_preference', { key })
}
export function updatePreferences(payload: Record<string, unknown>) {
  return commandInvoke<Record<string, unknown>>('update_preferences', { payload })
}

async function commandInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const started = performance.now()
  logger.debug('command start', { scope: 'tauri', command, args })
  try {
    const result = await invoke<T>(command, args)
    logger.debug('command success', {
      scope: 'tauri',
      command,
      durationMs: Math.round(performance.now() - started),
    })
    return result
  } catch (error) {
    logger.error('command failed', {
      scope: 'tauri',
      command,
      durationMs: Math.round(performance.now() - started),
      error: describeError(error),
      raw: error,
    })
    throw error
  }
}
