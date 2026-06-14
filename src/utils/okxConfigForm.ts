import type { OkxConfig, OkxConfigSaveRequest } from '@/types/system'

type OkxCredentialValues = {
  api_key: string
  secret_key: string
  passphrase: string
}

export type OkxCredentialKey = keyof OkxCredentialValues

export type OkxCredentialState = OkxCredentialValues & {
  is_configured: boolean
  masked: OkxCredentialValues
}

export type OkxConfigDraft = {
  use_simulated: boolean
  proxy_url: string
  effective_proxy_url: string
  demo: OkxCredentialState
  live: OkxCredentialState
}

export function createOkxConfigDraft(): OkxConfigDraft {
  return {
    use_simulated: true,
    proxy_url: '',
    effective_proxy_url: '',
    demo: emptyCredentialState(),
    live: emptyCredentialState(),
  }
}

export function applyOkxConfigToDraft(draft: OkxConfigDraft, config: OkxConfig | null) {
  if (!config) return
  draft.use_simulated = config.use_simulated !== false
  draft.proxy_url = readString(config.proxy_url)
  draft.effective_proxy_url = readString(config.effective_proxy_url)
  Object.assign(draft.demo, readCredentialState(config.demo))
  Object.assign(draft.live, readCredentialState(config.live))
}

export function okxConfigSavePayload(draft: OkxConfigDraft): OkxConfigSaveRequest {
  return {
    use_simulated: draft.use_simulated,
    demo: credentialPayload(draft.demo),
    live: credentialPayload(draft.live),
    proxy_url: draft.proxy_url.trim(),
  }
}

export function okxProxyLabel(draft: OkxConfigDraft) {
  return draft.proxy_url.trim() || draft.effective_proxy_url
}

export function placeholderForCredential(
  credentials: OkxCredentialState,
  defaultPlaceholder: string,
) {
  return hasMaskedValue(credentials) ? '留空保留当前值' : defaultPlaceholder
}

function emptyCredentialState(): OkxCredentialState {
  return {
    api_key: '',
    secret_key: '',
    passphrase: '',
    is_configured: false,
    masked: {
      api_key: '',
      secret_key: '',
      passphrase: '',
    },
  }
}

function readCredentialState(value: unknown): OkxCredentialState {
  const obj = isRecord(value) ? value : {}
  const masked = {
    api_key: readString(obj.api_key),
    secret_key: readString(obj.secret_key),
    passphrase: readString(obj.passphrase),
  }
  return {
    api_key: '',
    secret_key: '',
    passphrase: '',
    is_configured: readConfigured(obj, masked),
    masked,
  }
}

function credentialPayload(credentials: OkxCredentialState): OkxCredentialValues {
  return {
    api_key: credentials.api_key,
    secret_key: credentials.secret_key,
    passphrase: credentials.passphrase,
  }
}

function hasMaskedValue(credentials: OkxCredentialState) {
  return Boolean(
    credentials.masked.api_key ||
    credentials.masked.secret_key ||
    credentials.masked.passphrase,
  )
}

function readConfigured(source: Record<string, unknown>, credentials: OkxCredentialValues) {
  if (typeof source.is_configured === 'boolean') return source.is_configured
  return Boolean(credentials.api_key && credentials.secret_key && credentials.passphrase)
}

function readString(value: unknown) {
  return typeof value === 'string' ? value : ''
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
