export interface SystemStatus {
  healthy: boolean
  uptime: number
  db: { connected: boolean; size_mb: number }
  okx: { configured: boolean; connected: boolean; mode: string }
  memory: { used_mb: number; total_mb: number }
}

export interface SystemConfig {
  okx_configured: boolean
  assistant_configured: boolean
  use_simulated: boolean
  api_host: string
  api_port: number
  version: string
}

export interface OkxCredentialInput {
  api_key?: string
  secret_key?: string
  passphrase?: string
}

export interface OkxMaskedCredentials {
  api_key: string
  secret_key: string
  passphrase: string
  is_configured: boolean
}

export interface OkxConfig {
  demo: OkxMaskedCredentials
  live: OkxMaskedCredentials
  use_simulated: boolean
  is_configured: boolean
  proxy_url: string
  effective_proxy_url: string
}

export interface OkxConfigSaveRequest {
  demo?: OkxCredentialInput
  live?: OkxCredentialInput
  use_simulated: boolean
  proxy_url?: string
}

export interface OkxWebsocketDiagnostic {
  label?: string
  url?: string
  success?: boolean
  status?: number
  latency_ms?: number
  proxy?: string
  error?: string
}

export interface OkxConfigTestResult {
  success: boolean
  message: string
  data?: {
    mode?: string
    private_api?: boolean
    rest_success?: boolean
    endpoint?: string
    latency_ms?: number
    proxy?: string
    websocket?: Record<string, OkxWebsocketDiagnostic>
  }
}

export interface AssistantConfig {
  enabled: boolean
  configured: boolean
  base_url: string
  api_key: string
  model: string
  provider_name: string
}

export interface AssistantConfigSaveRequest {
  enabled: boolean
  base_url?: string
  api_key?: string
  model?: string
  provider_name?: string
}
