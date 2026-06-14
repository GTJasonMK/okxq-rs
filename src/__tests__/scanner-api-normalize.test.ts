import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { createProfile, fetchConditions, fetchProfiles, fetchResults, runScan } from '@/api/scanner'

const invokeMock = vi.mocked(invoke)

describe('扫描器 API 归一化', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('读取 profile/result/condition 的直接 snake_case 后端字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            profile_id: 'profile-1',
            name: '高分扫描',
            conditions: [
              { indicator: 'rsi' },
              { indicator: 'price' },
            ],
            symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
            inst_type: 'SWAP',
            timeframe: '15m',
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            profile_id: 'profile-1',
            inst_id: 'BTC-USDT-SWAP',
            matched_conditions: ['rsi', 'price'],
            indicator_values: { price: 70000, rsi: '78' },
            score: '110',
            scan_time: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            indicator: 'rsi',
            label: 'RSI',
            value_hint: '0-100',
          },
        ],
      })

    await expect(fetchProfiles()).resolves.toEqual([
      expect.objectContaining({
        id: 'profile-1',
        conditions: ['rsi', 'price'],
        symbol_filter: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
        inst_type: 'SWAP',
        timeframe: '15m',
        created_at: '2026-05-28T00:00:00.000Z',
      }),
    ])

    await expect(fetchResults()).resolves.toEqual([
      {
        id: 'BTC-USDT-SWAP-2026-05-28T00:00:00.000Z',
        profile_id: 'profile-1',
        symbol: 'BTC-USDT-SWAP',
        matched_conditions: ['rsi', 'price'],
        score: 100,
        details: { price: 70000, rsi: '78' },
        scanned_at: '2026-05-28T00:00:00.000Z',
      },
    ])

    await expect(fetchConditions()).resolves.toEqual([
      expect.objectContaining({
        id: 'rsi',
        name: 'RSI',
        description: '0-100',
      }),
    ])
  })

  it('运行扫描读取 results 和 scanned/matched 包络', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        results: [
          {
            inst_id: 'ETH-USDT-SWAP',
            matched_conditions: ['momentum'],
            indicator_values: { price: 3500 },
            scan_time: '2026-05-28T00:00:00.000Z',
          },
        ],
        scanned: 5,
        matched: 1,
      },
    })

    await expect(runScan({ conditions: ['momentum'] })).resolves.toMatchObject({
      scanned: 5,
      matched: 1,
      results: [
        {
          symbol: 'ETH-USDT-SWAP',
          matched_conditions: ['momentum'],
          score: 65,
          details: { price: 3500 },
        },
      ],
    })
  })

  it('创建扫描配置时写入真实布尔和统一 interval_seconds', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        profile_id: 'profile-2',
        name: '新扫描',
        conditions: [{ indicator: 'rsi' }],
      },
    })

    await expect(createProfile({
      name: '新扫描',
      conditions: ['rsi'],
      enabled: false,
      interval_seconds: 120,
    })).resolves.toMatchObject({
      id: 'profile-2',
      conditions: ['rsi'],
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: expect.objectContaining({
        method: 'POST',
        path: '/api/scanner/profiles',
        body: expect.objectContaining({
          enabled: false,
          interval_seconds: 120,
          conditions: [
            { indicator: 'rsi', operator: 'lt', value: 70, params: { period: 14 } },
          ],
        }),
      }),
    })
    const body = (invokeMock.mock.calls[0]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(body).not.toHaveProperty('condition_ids')
    expect(body).not.toHaveProperty('symbol_filter')
  })

  it('创建扫描配置时不把字符串数字或字符串布尔转为 typed payload', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        profile_id: 'profile-3',
        name: '默认扫描',
        conditions: [{ indicator: 'price' }],
      },
    })

    await createProfile({
      name: '默认扫描',
      conditions: ['price'],
      enabled: 'false',
      interval_seconds: '120',
    })

    const body = (invokeMock.mock.calls[0]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(body).toMatchObject({
      enabled: true,
      interval_seconds: 300,
      conditions: [
        { indicator: 'price', operator: 'gt', value: 0, params: {} },
      ],
    })
  })

  it('列表接口不读取旧 wrapper 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({ profiles: [{ profile_id: 'legacy-profile' }] })
      .mockResolvedValueOnce({ results: [{ inst_id: 'BTC-USDT-SWAP' }] })
      .mockResolvedValueOnce({ conditions: [{ indicator: 'legacy-condition' }] })

    await expect(fetchProfiles()).resolves.toEqual([])
    await expect(fetchResults()).resolves.toEqual([])
    await expect(fetchConditions()).resolves.toEqual([])
  })
})
