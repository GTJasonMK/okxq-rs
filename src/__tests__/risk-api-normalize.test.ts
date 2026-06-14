import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { fetchDrawdown, fetchMetrics, fetchOverview, fetchRolling, fetchSnapshots } from '@/api/risk'

const invokeMock = vi.mocked(invoke)

describe('风险 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('只读取后端 risk endpoint 的 snake_case 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            mode: 'simulated',
            date: '2026-05-28',
            total_equity: 1000,
            spot_value: 300,
            contract_value: 200,
            cash_value: 500,
            positions: { BTC: 1 },
            metadata: { source: 'portfolio_snapshots' },
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          has_data: true,
          data_points: 12,
          var_95: 0.02,
          var_99: 0.05,
          parametric_var_95: 0.03,
          sharpe_ratio: 1.2,
          sortino_ratio: 1.6,
          max_drawdown: -0.12,
          max_drawdown_duration: 3,
          current_drawdown: -0.02,
          peak_equity: 1100,
          latest_equity: 1000,
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          dates: ['2026-05-27T00:00:00.000Z', '2026-05-28T00:00:00.000Z'],
          equities: [1080, 1000],
          max_drawdown: -0.2,
          max_drawdown_duration: 3,
          current_drawdown: -0.08,
          peak: 1250,
          series: [0, -0.08],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          dates: ['2026-05-28T00:00:00.000Z'],
          sharpe: [1.5, null],
          volatility: [0.2],
          var_95: [0.01],
        },
      })

    await expect(fetchSnapshots()).resolves.toEqual([
      {
        mode: 'simulated',
        date: '2026-05-28',
        total_equity: 1000,
        spot_value: 300,
        contract_value: 200,
        cash_value: 500,
        positions: { BTC: 1 },
        metadata: { source: 'portfolio_snapshots' },
        created_at: '2026-05-28T00:00:00.000Z',
      },
    ])

    await expect(fetchMetrics()).resolves.toMatchObject({
      has_data: true,
      var_95: 0.02,
      var_99: 0.05,
      parametric_var_95: 0.03,
      sharpe_ratio: 1.2,
      sortino_ratio: 1.6,
      max_drawdown: -0.12,
      data_points: 12,
    })

    await expect(fetchDrawdown()).resolves.toMatchObject({
      dates: ['2026-05-27T00:00:00.000Z', '2026-05-28T00:00:00.000Z'],
      equities: [1080, 1000],
      max_drawdown: -0.2,
      max_drawdown_duration: 3,
      current_drawdown: -0.08,
      peak: 1250,
      series: [
        { time: 1779840000, value: 0 },
        { time: 1779926400, value: -0.08 },
      ],
    })

    await expect(fetchRolling()).resolves.toMatchObject({
      dates: ['2026-05-28T00:00:00.000Z'],
      var_95: [0.01],
      sharpe: [1.5],
      volatility: [0.2],
      benchmark: [{ time: 1779926400, value: 1.5 }],
    })
  })

  it('风险总览只请求 overview endpoint 并复用现有 snake_case normalizer', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        snapshots: [
          {
            mode: 'simulated',
            date: '2026-05-28',
            total_equity: 1000,
            spot_value: 300,
            contract_value: 200,
            cash_value: 500,
            positions: { BTC: 1 },
            metadata: { source: 'portfolio_snapshots' },
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
        metrics: {
          has_data: true,
          data_points: 12,
          var_95: 0.02,
          var_99: 0.05,
          parametric_var_95: 0.03,
          sharpe_ratio: 1.2,
          sortino_ratio: 1.6,
          max_drawdown: -0.12,
          max_drawdown_duration: 3,
          current_drawdown: -0.02,
          peak_equity: 1100,
          latest_equity: 1000,
        },
        drawdown: {
          dates: ['2026-05-27T00:00:00.000Z', '2026-05-28T00:00:00.000Z'],
          equities: [1080, 1000],
          max_drawdown: -0.2,
          max_drawdown_duration: 3,
          current_drawdown: -0.08,
          peak: 1250,
          series: [0, -0.08],
        },
        rolling: {
          dates: ['2026-05-28T00:00:00.000Z'],
          sharpe: [1.5],
          volatility: [0.2],
          var_95: [0.01],
        },
      },
    })

    await expect(fetchOverview()).resolves.toMatchObject({
      snapshots: [
        {
          mode: 'simulated',
          total_equity: 1000,
          spot_value: 300,
          contract_value: 200,
          cash_value: 500,
        },
      ],
      metrics: {
        has_data: true,
        var_95: 0.02,
        sharpe_ratio: 1.2,
        data_points: 12,
      },
      drawdown: {
        max_drawdown: -0.2,
        series: [
          { time: 1779840000, value: 0 },
          { time: 1779926400, value: -0.08 },
        ],
      },
      rolling: {
        sharpe: [1.5],
        benchmark: [{ time: 1779926400, value: 1.5 }],
      },
    })
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock.mock.calls[0]?.[1]).toMatchObject({
      req: { method: 'GET', path: '/api/risk/overview' },
    })
  })

  it('不会把字符串数字或字符串布尔值当作 typed risk 数据', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            mode: 'simulated',
            date: '2026-05-28',
            total_equity: '1000',
            spot_value: '300',
            contract_value: '200',
            cash_value: '500',
            positions: {},
            metadata: {},
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          has_data: 'true',
          data_points: '12',
          var_95: '0.02',
          var_99: '0.05',
          parametric_var_95: '0.03',
          sharpe_ratio: '1.2',
          sortino_ratio: '1.6',
          max_drawdown: '-0.12',
          max_drawdown_duration: '3',
          current_drawdown: '-0.02',
          peak_equity: '1100',
          latest_equity: '1000',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          dates: ['2026-05-28T00:00:00.000Z'],
          equities: ['1000'],
          max_drawdown: '-0.2',
          max_drawdown_duration: '3',
          current_drawdown: '-0.08',
          peak: '1250',
          series: ['0'],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          dates: ['2026-05-28T00:00:00.000Z'],
          sharpe: ['1.5'],
          volatility: ['0.2'],
          var_95: ['0.01'],
        },
      })

    await expect(fetchSnapshots()).resolves.toEqual([])

    await expect(fetchMetrics()).resolves.toMatchObject({
      has_data: false,
      data_points: null,
      var_95: null,
      sharpe_ratio: null,
    })

    await expect(fetchDrawdown()).resolves.toMatchObject({
      equities: [],
      max_drawdown: null,
      series: [],
    })

    await expect(fetchRolling()).resolves.toMatchObject({
      sharpe: [],
      volatility: [],
      var_95: [],
      benchmark: [],
    })
  })

  it('风险指标有数据时不把非法标量伪装成 0', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          has_data: true,
          data_points: 12,
          var_95: '0.02',
          var_99: '0.05',
          parametric_var_95: '0.03',
          sharpe_ratio: '1.2',
          sortino_ratio: '1.6',
          max_drawdown: '-0.12',
          max_drawdown_duration: '3',
          current_drawdown: '-0.02',
          peak_equity: '1100',
          latest_equity: '1000',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          dates: ['2026-05-28T00:00:00.000Z'],
          equities: [1000],
          max_drawdown: '-0.2',
          max_drawdown_duration: '3',
          current_drawdown: '-0.08',
          peak: '1250',
          series: ['0'],
        },
      })

    await expect(fetchMetrics()).resolves.toMatchObject({
      has_data: true,
      data_points: 12,
      var_95: null,
      var_99: null,
      parametric_var_95: null,
      sharpe_ratio: null,
      sortino_ratio: null,
      max_drawdown: null,
      max_drawdown_duration: null,
      current_drawdown: null,
      peak_equity: null,
      latest_equity: null,
    })

    await expect(fetchDrawdown()).resolves.toMatchObject({
      equities: [1000],
      max_drawdown: null,
      max_drawdown_duration: null,
      current_drawdown: null,
      peak: null,
      series: [],
    })
  })

  it('滚动指标没有有效序列时不生成 0 摘要行', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        dates: ['2026-05-28T00:00:00.000Z'],
        sharpe: ['1.5'],
        volatility: [],
        var_95: [],
      },
    })

    await expect(fetchRolling()).resolves.toMatchObject({
      sharpe: [],
      volatility: [],
      var_95: [],
      metrics: [],
      benchmark: [],
    })
  })

  it('风险快照不读取旧 wrapper 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      snapshots: [
        {
          mode: 'simulated',
          date: '2026-05-28',
          total_equity: 1000,
        },
      ],
    })

    await expect(fetchSnapshots()).resolves.toEqual([])
  })

  it('回撤图表序列按原始下标匹配日期，忽略无效数值但不前移时间', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        dates: ['2026-05-27T00:00:00.000Z', '2026-05-28T00:00:00.000Z'],
        equities: [1080, 1000],
        max_drawdown: -0.2,
        max_drawdown_duration: 3,
        current_drawdown: -0.08,
        peak: 1250,
        series: [null, -0.08],
      },
    })

    await expect(fetchDrawdown()).resolves.toMatchObject({
      series: [
        { time: 1779926400, value: -0.08 },
      ],
    })
  })

  it('回撤图表序列忽略无效数值对应的无效日期，不压缩日期数组', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        dates: [null, '2026-05-28T00:00:00.000Z'],
        equities: [1080, 1000],
        max_drawdown: -0.2,
        max_drawdown_duration: 3,
        current_drawdown: -0.08,
        peak: 1250,
        series: [null, -0.08],
      },
    })

    await expect(fetchDrawdown()).resolves.toMatchObject({
      series: [
        { time: 1779926400, value: -0.08 },
      ],
    })
  })
})
