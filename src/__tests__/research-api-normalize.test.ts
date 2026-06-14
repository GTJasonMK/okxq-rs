import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  computeFactors,
  fetchCollectionSessions,
  fetchDatasets,
  fetchTrainingRuns,
  fetchTrendConfig,
  trainModel,
  updateTrendConfig,
} from '@/api/research'

const invokeMock = vi.mocked(invoke)

describe('研究 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('集合、数据集和训练记录只消费直接数组 payload', async () => {
    invokeMock
      .mockResolvedValueOnce([
        {
          session_id: 'session-1',
          status: 'finished',
        },
      ])
      .mockResolvedValueOnce([
        {
          dataset_id: 'ds-1',
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          status: 'ready',
          created_at: 1779926400,
          updated_at: 1779926400,
        },
      ])
      .mockResolvedValueOnce([
        {
          run_id: 'run-1',
          dataset_id: 'ds-1',
          progress_stage: 'completed',
          metrics: {
            val: {
              r_squared: 0.12,
              mse: 0.01,
              mae: 0.02,
              direction_accuracy: 0.61,
            },
          },
          created_at: 1779926400,
        },
      ])

    await expect(fetchCollectionSessions()).resolves.toEqual([
      {
        session_id: 'session-1',
        status: 'finished',
      },
    ])
    await expect(fetchDatasets()).resolves.toEqual([
      expect.objectContaining({
        id: 'ds-1',
        dataset_id: 'ds-1',
        name: 'ds-1',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        status: 'ready',
        created_at: '2026-05-28T00:00:00.000Z',
        updated_at: '2026-05-28T00:00:00.000Z',
      }),
    ])
    await expect(fetchTrainingRuns()).resolves.toEqual([
      expect.objectContaining({
        id: 'run-1',
        run_id: 'run-1',
        dataset_id: 'ds-1',
        progress_stage: 'completed',
        r2: 0.12,
        mse: 0.01,
        mae: 0.02,
        direction_accuracy: 0.61,
        created_at: '2026-05-28T00:00:00.000Z',
      }),
    ])
  })

  it('训练结果和因子计算只消费直接对象或数组 payload', async () => {
    invokeMock
      .mockResolvedValueOnce({
        run_id: 'run-2',
        dataset_id: 'ds-2',
        metrics: {
          val: {
            r_squared: 0.23,
            direction_accuracy: 0.7,
          },
          test: {
            mse: 0.03,
            mae: 0.04,
          },
        },
      })
      .mockResolvedValueOnce([
        { factor_name: 'momentum', value: 0.15 },
        { factor_name: 'spread', value: -0.02 },
      ])

    await expect(trainModel('ds-2')).resolves.toMatchObject({
      id: 'run-2',
      dataset_id: 'ds-2',
      r2: 0.23,
      mse: 0.03,
      mae: 0.04,
      direction_accuracy: 0.7,
    })

    await expect(computeFactors('BTC-USDT-SWAP', 120, { inst_type: 'SWAP', timeframe: '1H' })).resolves.toEqual([
      { name: 'momentum', value: 0.15 },
      { name: 'spread', value: -0.02 },
    ])
  })

  it('趋势配置读取和保存只使用直接 snake_case 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        whitelist: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
        enabled: true,
      })
      .mockResolvedValueOnce({
        ok: true,
      })

    await expect(fetchTrendConfig()).resolves.toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      bar_count: 500,
      enabled: true,
    })

    await updateTrendConfig({
      symbol: 'btc',
      inst_type: 'SWAP',
      enabled: false,
    })

    expect(invokeMock).toHaveBeenLastCalledWith('local_api_request', {
      req: expect.objectContaining({
        method: 'PUT',
        path: '/api/trend-research/config',
        body: expect.objectContaining({
          inst_type: 'SWAP',
          enabled: false,
          whitelist: ['BTC-USDT-SWAP'],
        }),
      }),
    })
    const body = (invokeMock.mock.calls[1]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(body).not.toHaveProperty('symbol')
    expect(body).not.toHaveProperty('bar_count')
  })

  it('不读取旧 wrapper，也不解析字符串数字、字符串布尔或非字符串列表项', async () => {
    invokeMock
      .mockResolvedValueOnce({
        datasets: [
          {
            dataset_id: 'legacy-ds',
          },
        ],
      })
      .mockResolvedValueOnce({
        training_runs: [
          {
            run_id: 'legacy-run',
          },
        ],
      })
      .mockResolvedValueOnce({
        training_run: {
          run_id: 'legacy-run',
          metrics: { val: { r_squared: '0.23' } },
        },
      })
      .mockResolvedValueOnce({
        factors: [
          { factor_name: 'momentum', value: '0.15' },
        ],
      })
      .mockResolvedValueOnce({
        config: {
          whitelist: ['BTC-USDT-SWAP'],
          enabled: true,
        },
        enabled: 'true',
      })
      .mockResolvedValueOnce({ ok: true })

    await expect(fetchDatasets()).resolves.toEqual([])
    await expect(fetchTrainingRuns()).resolves.toEqual([])
    await expect(trainModel('ds-2')).resolves.toMatchObject({
      id: '',
      dataset_id: '',
      r2: 0,
    })
    await expect(computeFactors('BTC-USDT-SWAP')).resolves.toEqual([])
    await expect(fetchTrendConfig()).resolves.toMatchObject({
      symbol: '',
      enabled: false,
      whitelist: [],
    })

    await updateTrendConfig({
      symbol: 123,
      inst_type: 456,
      enabled: 'false',
      whitelist: [789, 'ETH-USDT-SWAP', ''],
      feature_bar_seconds: '1',
      state_sync_seconds: Number.NaN,
    })

    expect(invokeMock).toHaveBeenLastCalledWith('local_api_request', {
      req: expect.objectContaining({
        method: 'PUT',
        path: '/api/trend-research/config',
        body: {
          inst_type: 'SWAP',
          whitelist: ['ETH-USDT-SWAP'],
        },
      }),
    })
  })
})
