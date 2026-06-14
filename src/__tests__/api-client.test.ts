import { afterEach, describe, it, expect, vi } from 'vitest'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { ApiError } from '@/types/api'
import { apiGet } from '@/api/client'

const invokeMock = vi.mocked(invoke)
const isTauriMock = vi.mocked(isTauri)

describe('API 客户端基础', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
    isTauriMock.mockReturnValue(true)
  })

  it('ApiError 正确创建', () => {
    const err = new ApiError('测试错误', 500)
    expect(err.message).toBe('测试错误')
    expect(err.code).toBe(500)
    expect(err.name).toBe('ApiError')
  })

  it('并发相同 GET 请求只调用一次 Tauri 命令', async () => {
    let resolveRequest: (value: unknown) => void = () => {}
    invokeMock.mockReturnValueOnce(new Promise(resolve => {
      resolveRequest = resolve
    }))

    const first = apiGet('/api/market/tickers', { b: 2, a: 1 })
    const second = apiGet('/api/market/tickers', { a: 1, b: 2 })

    expect(invokeMock).toHaveBeenCalledTimes(1)
    resolveRequest({ code: 0, data: ['ok'] })

    await expect(first).resolves.toEqual(['ok'])
    await expect(second).resolves.toEqual(['ok'])
  })

  it('纯前端页面直接给出桌面端启动提示，不调用 Tauri 命令', async () => {
    isTauriMock.mockReturnValueOnce(false)

    await expect(apiGet('/api/trading/account')).rejects.toThrow(
      '当前页面没有运行在 Tauri 桌面端',
    )
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('结构化错误会展开阻塞对象 ID', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 409,
      message: '删除数据集失败',
      data: {
        blocking_training_run_ids: ['run_a', 'run_b'],
      },
    })

    await expect(apiGet('/api/research-platform/datasets/ds_1')).rejects.toMatchObject({
      name: 'ApiError',
      code: 409,
      message: '删除数据集失败：run_a、run_b',
    })
  })

  it('只把数值 code 识别为响应包络', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 409,
      message: '数值错误码',
    })
    await expect(apiGet('/api/code-error')).rejects.toMatchObject({
      name: 'ApiError',
      code: 409,
      message: '数值错误码',
    })

    invokeMock.mockResolvedValueOnce({
      success: false,
      message: '布尔失败状态',
    })
    await expect(apiGet('/api/success-false')).resolves.toEqual({
      success: false,
      message: '布尔失败状态',
    })
  })

  it('字符串 code 和 success 字段不按包络解包', async () => {
    invokeMock.mockResolvedValueOnce({
      code: '0',
      data: { ok: true },
    })
    await expect(apiGet('/api/string-code-ok')).resolves.toEqual({
      code: '0',
      data: { ok: true },
    })

    invokeMock.mockResolvedValueOnce({
      success: 'true',
      data: ['ok'],
    })
    await expect(apiGet('/api/string-success-ok')).resolves.toEqual({
      success: 'true',
      data: ['ok'],
    })
  })
})
