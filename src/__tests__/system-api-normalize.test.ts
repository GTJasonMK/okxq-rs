import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  fetchAssistantConfig,
  fetchOkxConfig,
  saveAssistantConfig,
  saveOkxConfig,
  testOkxConfig,
} from '@/api/system'

const invokeMock = vi.mocked(invoke)

describe('系统配置 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('OKX 配置读取只消费后端 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      use_simulated: false,
      is_configured: true,
      proxy_url: ' http://127.0.0.1:7897 ',
      effective_proxy_url: 'DIRECT',
      demo: {
        api_key: '***demo-key',
        secret_key: '***demo-secret',
        passphrase: '***demo-pass',
        is_configured: false,
      },
      live: {
        api_key: '***live-key',
        secret_key: '***live-secret',
        passphrase: '***live-pass',
        is_configured: true,
      },
    })

    await expect(fetchOkxConfig()).resolves.toEqual({
      use_simulated: false,
      is_configured: true,
      proxy_url: ' http://127.0.0.1:7897 ',
      effective_proxy_url: 'DIRECT',
      demo: {
        api_key: '***demo-key',
        secret_key: '***demo-secret',
        passphrase: '***demo-pass',
        is_configured: false,
      },
      live: {
        api_key: '***live-key',
        secret_key: '***live-secret',
        passphrase: '***live-pass',
        is_configured: true,
      },
    })
  })

  it('保存 OKX 配置时只发送 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({ success: true })

    const payload = {
      use_simulated: false,
      proxy_url: ' http://127.0.0.1:7897 ',
      demo: {
        api_key: ' demo-key ',
        secret_key: ' demo-secret ',
        passphrase: ' demo-pass ',
      },
      live: {
        api_key: ' live-key ',
        secret_key: ' live-secret ',
        passphrase: ' live-pass ',
      },
    }

    await saveOkxConfig(payload)

    expect(invokeMock).toHaveBeenCalledWith('save_okx_config', {
      req: {
        use_simulated: false,
        proxy_url: 'http://127.0.0.1:7897',
        demo: {
          api_key: 'demo-key',
          secret_key: 'demo-secret',
          passphrase: 'demo-pass',
        },
        live: {
          api_key: 'live-key',
          secret_key: 'live-secret',
          passphrase: 'live-pass',
        },
      },
    })
  })

  it('OKX 测试结果只消费 snake_case 诊断字段', async () => {
    invokeMock.mockResolvedValueOnce({
      success: false,
      message: '连接失败',
      data: {
        private_api: false,
        rest_success: false,
        latency_ms: 120,
        proxy: '',
        websocket: {
          public: {
            success: true,
            latency_ms: 30,
          },
          business: {
            success: false,
            error: 'timeout',
          },
        },
      },
    })

    await expect(testOkxConfig({
      use_simulated: true,
      demo: { api_key: '', secret_key: '', passphrase: '' },
      live: { api_key: '', secret_key: '', passphrase: '' },
    })).resolves.toEqual({
      success: false,
      message: '连接失败',
      data: {
        mode: undefined,
        private_api: false,
        rest_success: false,
        endpoint: undefined,
        latency_ms: 120,
        proxy: undefined,
        websocket: {
          public: {
            label: undefined,
            url: undefined,
            success: true,
            status: undefined,
            latency_ms: 30,
            proxy: undefined,
            error: undefined,
          },
          business: {
            label: undefined,
            url: undefined,
            success: false,
            status: undefined,
            latency_ms: undefined,
            proxy: undefined,
            error: 'timeout',
          },
        },
      },
    })
  })

  it('AI 助手配置读取只消费后端 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      enabled: false,
      configured: true,
      base_url: ' https://api.openai.com/v1 ',
      api_key: '***masked',
      model: 'gpt-4.1-mini',
      provider_name: 'OpenAI',
    })

    await expect(fetchAssistantConfig()).resolves.toEqual({
      enabled: false,
      configured: true,
      base_url: ' https://api.openai.com/v1 ',
      api_key: '***masked',
      model: 'gpt-4.1-mini',
      provider_name: 'OpenAI',
    })
  })

  it('保存 AI 助手配置时只发送 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({ success: true })

    await saveAssistantConfig({
      enabled: false,
      base_url: ' https://api.openai.com/v1 ',
      api_key: ' sk-test ',
      model: ' gpt-4.1-mini ',
      provider_name: ' OpenAI ',
    })

    expect(invokeMock).toHaveBeenCalledWith('save_assistant_config', {
      req: {
        enabled: false,
        base_url: 'https://api.openai.com/v1',
        api_key: 'sk-test',
        model: 'gpt-4.1-mini',
        provider_name: 'OpenAI',
      },
    })
  })

  it('配置读写不解析字符串数字、字符串布尔或非字符串密钥', async () => {
    invokeMock.mockResolvedValueOnce({
      use_simulated: 'false',
      is_configured: 'true',
      proxy_url: 123,
      effective_proxy_url: true,
      demo: {
        api_key: 123,
        secret_key: '***demo-secret',
        passphrase: '***demo-pass',
        is_configured: 'true',
      },
      live: {
        api_key: '***live-key',
        secret_key: '***live-secret',
        passphrase: '***live-pass',
        is_configured: 'false',
      },
    })

    await expect(fetchOkxConfig()).resolves.toEqual({
      use_simulated: true,
      is_configured: false,
      proxy_url: '',
      effective_proxy_url: '',
      demo: {
        api_key: '',
        secret_key: '***demo-secret',
        passphrase: '***demo-pass',
        is_configured: false,
      },
      live: {
        api_key: '***live-key',
        secret_key: '***live-secret',
        passphrase: '***live-pass',
        is_configured: true,
      },
    })

    invokeMock.mockResolvedValueOnce({ success: true })
    await saveOkxConfig({
      use_simulated: 'false' as unknown as boolean,
      proxy_url: 123 as unknown as string,
      demo: {
        api_key: 123 as unknown as string,
        secret_key: ' demo-secret ',
        passphrase: true as unknown as string,
      },
      live: undefined,
    })

    expect(invokeMock).toHaveBeenLastCalledWith('save_okx_config', {
      req: {
        use_simulated: true,
        proxy_url: '',
        demo: {
          api_key: '',
          secret_key: 'demo-secret',
          passphrase: '',
        },
        live: undefined,
      },
    })

    invokeMock.mockResolvedValueOnce({
      success: 'true',
      message: 404,
      data: {
        private_api: 'false',
        rest_success: 'true',
        latency_ms: '120',
        proxy: 7897,
        websocket: {
          public: {
            success: 'true',
            status: '200',
            latency_ms: '30',
            error: 500,
          },
        },
      },
    })

    await expect(testOkxConfig({
      use_simulated: true,
      demo: { api_key: '', secret_key: '', passphrase: '' },
      live: { api_key: '', secret_key: '', passphrase: '' },
    })).resolves.toEqual({
      success: false,
      message: '',
      data: {
        mode: undefined,
        private_api: undefined,
        rest_success: undefined,
        endpoint: undefined,
        latency_ms: undefined,
        proxy: undefined,
        websocket: {
          public: {
            label: undefined,
            url: undefined,
            success: undefined,
            status: undefined,
            latency_ms: undefined,
            proxy: undefined,
            error: undefined,
          },
        },
      },
    })

    invokeMock.mockResolvedValueOnce({ success: true })
    await saveAssistantConfig({
      enabled: 'false' as unknown as boolean,
      base_url: 123 as unknown as string,
      api_key: true as unknown as string,
      model: ' gpt-4.1-mini ',
      provider_name: null as unknown as string,
    })

    expect(invokeMock).toHaveBeenLastCalledWith('save_assistant_config', {
      req: {
        enabled: true,
        base_url: '',
        api_key: '',
        model: 'gpt-4.1-mini',
        provider_name: '',
      },
    })
  })
})
