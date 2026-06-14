import type * as T from '@/types/system'
import {
  booleanValue,
  isRecord,
  stringValue,
} from '../normalize'

export function normalizeOkxConfigRequest(data: T.OkxConfigSaveRequest): T.OkxConfigSaveRequest {
  return {
    demo: normalizeCredentials(data.demo),
    live: normalizeCredentials(data.live),
    use_simulated: booleanValue(data.use_simulated, true),
    proxy_url: stringValue(data.proxy_url).trim(),
  }
}

export function normalizeOkxConfigResponse(raw: unknown): T.OkxConfig {
  const item = isRecord(raw) ? raw : {}
  return {
    demo: normalizeMaskedCredentials(item.demo),
    live: normalizeMaskedCredentials(item.live),
    use_simulated: booleanValue(item.use_simulated, true),
    is_configured: booleanValue(item.is_configured),
    proxy_url: stringValue(item.proxy_url),
    effective_proxy_url: stringValue(item.effective_proxy_url),
  }
}

export function normalizeOkxTestResult(raw: unknown): T.OkxConfigTestResult {
  const item = isRecord(raw) ? raw : {}
  const data = isRecord(item.data) ? item.data : {}
  return {
    success: booleanValue(item.success),
    message: stringValue(item.message),
    data: {
      mode: optionalString(data.mode),
      private_api: optionalBoolean(data.private_api),
      rest_success: optionalBoolean(data.rest_success),
      endpoint: optionalString(data.endpoint),
      latency_ms: optionalNumber(data.latency_ms),
      proxy: optionalString(data.proxy),
      websocket: normalizeWebsocketDiagnostics(data.websocket),
    },
  }
}

export function normalizeAssistantConfigRequest(data: T.AssistantConfigSaveRequest): T.AssistantConfigSaveRequest {
  return {
    enabled: booleanValue(data.enabled, true),
    base_url: stringValue(data.base_url).trim(),
    api_key: stringValue(data.api_key).trim(),
    model: stringValue(data.model).trim(),
    provider_name: stringValue(data.provider_name).trim(),
  }
}

export function normalizeAssistantConfigResponse(raw: unknown): T.AssistantConfig {
  const item = isRecord(raw) ? raw : {}
  return {
    enabled: booleanValue(item.enabled, true),
    configured: booleanValue(item.configured),
    base_url: stringValue(item.base_url),
    api_key: stringValue(item.api_key),
    model: stringValue(item.model),
    provider_name: stringValue(item.provider_name),
  }
}

function normalizeCredentials(value: unknown): T.OkxCredentialInput | undefined {
  if (!isRecord(value)) return undefined
  return {
    api_key: stringValue(value.api_key).trim(),
    secret_key: stringValue(value.secret_key).trim(),
    passphrase: stringValue(value.passphrase).trim(),
  }
}

function normalizeMaskedCredentials(value: unknown): T.OkxMaskedCredentials {
  const item = isRecord(value) ? value : {}
  const masked = {
    api_key: stringValue(item.api_key),
    secret_key: stringValue(item.secret_key),
    passphrase: stringValue(item.passphrase),
  }
  return {
    ...masked,
    is_configured: booleanValue(
      item.is_configured,
      Boolean(masked.api_key && masked.secret_key && masked.passphrase),
    ),
  }
}

function normalizeWebsocketDiagnostics(value: unknown): Record<string, T.OkxWebsocketDiagnostic> {
  if (!isRecord(value)) return {}
  return Object.fromEntries(
    Object.entries(value).map(([key, raw]) => {
      const item = isRecord(raw) ? raw : {}
      return [key, {
        label: optionalString(item.label),
        url: optionalString(item.url),
        success: optionalBoolean(item.success),
        status: optionalNumber(item.status),
        latency_ms: optionalNumber(item.latency_ms),
        proxy: optionalString(item.proxy),
        error: optionalString(item.error),
      }]
    }),
  )
}

function optionalBoolean(value: unknown): boolean | undefined {
  if (value === undefined || value === null) return undefined
  return typeof value === 'boolean' ? value : undefined
}

function optionalNumber(value: unknown): number | undefined {
  if (value === undefined || value === null) return undefined
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function optionalString(value: unknown): string | undefined {
  const text = stringValue(value)
  return text ? text : undefined
}
