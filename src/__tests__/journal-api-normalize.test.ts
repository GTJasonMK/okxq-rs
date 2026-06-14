import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { createEntry, fetchEntries, fetchStats, fetchTags, updateEntry } from '@/api/journal'

const invokeMock = vi.mocked(invoke)

describe('交易日志 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('日志列表只读取后端 journal row 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          entry_id: 'je-1',
          title: 'BTC 复盘',
          content: '执行正常',
          mode: 'live',
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          trade_ids: ['trade-1'],
          order_ids: ['order-1'],
          tags: ['breakout', 'review'],
          strategy_id: 'v17',
          strategy_name: 'V17',
          rating: 7,
          emotion: 'calm',
          screenshots: [],
          pnl_snapshot: 12.5,
          metadata: { source: 'manual' },
          created_at: '2026-05-28T00:00:00.000Z',
          updated_at: '2026-05-28T00:01:00.000Z',
        },
      ],
    })

    await expect(fetchEntries()).resolves.toEqual([
      {
        id: 'je-1',
        entry_id: 'je-1',
        title: 'BTC 复盘',
        content: '执行正常',
        mode: 'live',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        trade_ids: ['trade-1'],
        order_ids: ['order-1'],
        tags: ['breakout', 'review'],
        strategy_id: 'v17',
        strategy_name: 'V17',
        rating: 5,
        emotion: 'calm',
        screenshots: [],
        pnl_snapshot: 12.5,
        metadata: { source: 'manual' },
        created_at: '2026-05-28T00:00:00.000Z',
        updated_at: '2026-05-28T00:01:00.000Z',
      },
    ])
  })

  it('日志列表过滤使用数组 tags，不发送逗号拼接字符串', async () => {
    invokeMock.mockResolvedValueOnce({ code: 0, data: [] })

    await expect(fetchEntries({ tags: ['review', 'breakout'], limit: 10 })).resolves.toEqual([])

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/journal/entries',
        params: {
          tags: ['review', 'breakout'],
          limit: 10,
        },
        body: undefined,
      },
    })
  })

  it('创建和更新日志时只发送后端规范字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          entry_id: 'je-2',
          title: '新日志',
          content: '执行说明',
          mode: 'simulated',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          trade_ids: [],
          order_ids: [],
          tags: ['mean-reversion'],
          strategy_id: '',
          strategy_name: 'V17',
          rating: 5,
          emotion: '',
          screenshots: [],
          pnl_snapshot: 5.5,
          metadata: {},
          created_at: '2026-05-28T00:00:00.000Z',
          updated_at: '2026-05-28T00:00:00.000Z',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          entry_id: 'je-2',
          title: '更新日志',
          content: '执行说明',
          mode: 'simulated',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          trade_ids: [],
          order_ids: [],
          tags: ['mean-reversion'],
          strategy_id: '',
          strategy_name: 'V20精选',
          rating: 5,
          emotion: '',
          screenshots: [],
          pnl_snapshot: 3.2,
          metadata: {},
          created_at: '2026-05-28T00:00:00.000Z',
          updated_at: '2026-05-28T00:01:00.000Z',
        },
      })

    await expect(createEntry({
      title: ' 新日志 ',
      content: '执行说明',
      inst_id: ' ETH-USDT-SWAP ',
      inst_type: 'SWAP',
      mode: 'simulated',
      rating: 4.6,
      tags: ['mean-reversion', ''],
      strategy_name: ' V17 ',
      pnl_snapshot: 5.5,
    })).resolves.toMatchObject({
      id: 'je-2',
      entry_id: 'je-2',
      title: '新日志',
      inst_id: 'ETH-USDT-SWAP',
      tags: ['mean-reversion'],
    })

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'local_api_request', {
      req: expect.objectContaining({
        method: 'POST',
        path: '/api/journal/entries',
        body: {
          title: '新日志',
          content: '执行说明',
          mode: 'simulated',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          strategy_name: 'V17',
          rating: 5,
          pnl_snapshot: 5.5,
          tags: ['mean-reversion'],
        },
      }),
    })

    await expect(updateEntry('je-2', {
      title: '更新日志',
      strategy_name: ' V20精选 ',
      pnl_snapshot: 3.2,
    })).resolves.toMatchObject({
      id: 'je-2',
      entry_id: 'je-2',
      title: '更新日志',
      strategy_name: 'V20精选',
      pnl_snapshot: 3.2,
      updated_at: '2026-05-28T00:01:00.000Z',
    })

    const updateBody = (invokeMock.mock.calls[1]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(updateBody).toEqual({
      title: '更新日志',
      strategy_name: 'V20精选',
      pnl_snapshot: 3.2,
    })
    expect(updateBody).not.toHaveProperty('symbol')
    expect(updateBody).not.toHaveProperty('strategy')
    expect(updateBody).not.toHaveProperty('strategyName')
  })

  it('标签和统计只读取后端 usage_count 与 groups 结构', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            tag: 'review',
            usage_count: 3,
            color: '#ff9800',
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          group_by: 'tag',
          total_entries: 3,
          groups: [
            {
              key: 'review',
              count: 3,
              total_pnl: 18.5,
              win_rate: 66.7,
              avg_rating: 4.3,
            },
          ],
        },
      })

    await expect(fetchTags()).resolves.toEqual([
      {
        tag: 'review',
        usage_count: 3,
        color: '#ff9800',
        created_at: '2026-05-28T00:00:00.000Z',
      },
    ])

    await expect(fetchStats()).resolves.toEqual({
      total_entries: 3,
      group_by: 'tag',
      groups: [
        {
          key: 'review',
          count: 3,
          total_pnl: 18.5,
          win_rate: 66.7,
          avg_rating: 4.3,
        },
      ],
    })
  })

  it('不读取旧 wrapper，也不解析字符串数字或非字符串列表项', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: { entries: [{ entry_id: 'legacy' }] },
    })
    await expect(fetchEntries()).resolves.toEqual([])

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          entry_id: 'je-strict',
          title: 'Strict',
          rating: '5',
          pnl_snapshot: '12.5',
          trade_ids: [123, 'trade-1'],
          tags: ['review', 456],
        },
      ],
    })

    await expect(fetchEntries()).resolves.toMatchObject([
      {
        entry_id: 'je-strict',
        rating: 0,
        pnl_snapshot: 0,
        trade_ids: ['trade-1'],
        tags: ['review'],
      },
    ])

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        entry_id: 'je-payload',
        title: 'Payload',
        content: '',
        mode: 'simulated',
        inst_id: '',
        inst_type: 'SPOT',
        trade_ids: [],
        order_ids: [],
        tags: [],
        strategy_id: '',
        strategy_name: '',
        rating: 0,
        emotion: '',
        screenshots: [],
        pnl_snapshot: 0,
        metadata: {},
        created_at: '2026-05-28T00:00:00.000Z',
        updated_at: '2026-05-28T00:00:00.000Z',
      },
    })

    await createEntry({
      title: 'Payload',
      rating: '5' as unknown as number,
      pnl_snapshot: '12.5' as unknown as number,
      tags: ['review', 456 as unknown as string],
      metadata: 'invalid' as unknown as Record<string, unknown>,
      created_at: 1779926400000 as unknown as string,
    })

    const body = (invokeMock.mock.calls[2]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(body).toEqual({
      title: 'Payload',
      tags: ['review'],
    })
  })
})
