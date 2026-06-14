import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  chat,
  createSession,
  fetchAgentCapabilities,
  fetchAssistantStatus,
  fetchOrderDrafts,
  fetchPatrolConfig,
  fetchPatrolStatus,
  fetchSession,
  fetchSessions,
  runPatrolNow,
  updatePatrolConfig,
} from '@/api/assistant'

const invokeMock = vi.mocked(invoke)

describe('Assistant API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('读取状态、工具、会话和详情时只接受后端 snake_case 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          enabled: false,
          configured: true,
          provider_name: 'OpenAI',
          model: 'gpt-4.1-mini',
          runtime: 'rust',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            name: 'market_context',
            description: '读取当前行情上下文',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            id: 's-1',
            session_id: 's-1',
            title: 'BTC 分析',
            mode: 'live',
            created_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          id: 's-2',
          session_id: 's-2',
          title: '新会话',
          mode: 'simulated',
          created_at: '2026-05-28T00:00:00.000Z',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          session: {
            id: 's-1',
            session_id: 's-1',
            title: 'BTC 分析',
            mode: 'live',
            created_at: '2026-05-28T00:00:00.000Z',
          },
          messages: [
            {
              id: 'm-1',
              message_id: 'm-1',
              session_id: 's-1',
              role: 'user',
              content: '你好',
              created_at: '2026-05-28T00:01:00.000Z',
            },
          ],
          steps: [
            {
              id: 'step-1',
              step_id: 'step-1',
              session_id: 's-1',
              step_type: 'tool',
              title: '读取行情',
              created_at: '2026-05-28T00:01:30.000Z',
            },
          ],
          order_drafts: [
            {
              id: 'd-1',
              draft_id: 'd-1',
              session_id: 's-1',
              inst_id: 'BTC-USDT-SWAP',
              mode: 'simulated',
              side: 'sell',
              order_type: 'limit',
              size: '0.1',
              price: '70000',
              status: 'draft',
              created_at: '2026-05-28T00:02:00.000Z',
              updated_at: '2026-05-28T00:02:00.000Z',
            },
          ],
          level_snapshots: [
            {
              id: 'lvl-1',
              snapshot_id: 'lvl-1',
              session_id: 's-1',
              inst_id: 'BTC-USDT-SWAP',
              mode: 'simulated',
              timeframes: ['15m'],
              supports: [],
              resistances: [],
              invalidation_levels: [],
              chart_annotations: [],
              summary: {},
              metadata: {},
              created_at: '2026-05-28T00:03:00.000Z',
            },
          ],
        },
      })

    await expect(fetchAssistantStatus()).resolves.toEqual({
      enabled: false,
      configured: true,
      provider_name: 'OpenAI',
      model: 'gpt-4.1-mini',
      runtime: 'rust',
    })

    await expect(fetchAgentCapabilities()).resolves.toEqual([
      {
        name: 'market_context',
        description: '读取当前行情上下文',
      },
    ])

    await expect(fetchSessions()).resolves.toEqual([
      {
        id: 's-1',
        session_id: 's-1',
        title: 'BTC 分析',
        mode: 'live',
        created_at: '2026-05-28T00:00:00.000Z',
      },
    ])

    await expect(createSession({
      title: ' 新会话 ',
      inst_id: ' BTC-USDT-SWAP ',
      inst_type: 'SWAP',
    })).resolves.toEqual({
      id: 's-2',
      session_id: 's-2',
      title: '新会话',
      mode: 'simulated',
      created_at: '2026-05-28T00:00:00.000Z',
    })

    const createBody = (invokeMock.mock.calls[3]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(createBody).toEqual({
      title: '新会话',
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      metadata: {},
    })
    expect(createBody).not.toHaveProperty('instId')
    expect(createBody).not.toHaveProperty('instType')

    await expect(fetchSession('s-1')).resolves.toMatchObject({
      session: {
        id: 's-1',
        session_id: 's-1',
      },
      messages: [
        {
          id: 'm-1',
          message_id: 'm-1',
          session_id: 's-1',
          role: 'user',
          content: '你好',
          created_at: '2026-05-28T00:01:00.000Z',
        },
      ],
      order_drafts: [
        {
          id: 'd-1',
          draft_id: 'd-1',
          inst_id: 'BTC-USDT-SWAP',
          side: 'sell',
          order_type: 'limit',
          size: '0.1',
          price: '70000',
          status: 'draft',
        },
      ],
      level_snapshots: [
        {
          id: 'lvl-1',
          snapshot_id: 'lvl-1',
          inst_id: 'BTC-USDT-SWAP',
          timeframes: ['15m'],
        },
      ],
    })
  })

  it('聊天返回只读取 assistant_message、session 和 detail.messages', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        session_id: 's-1',
        assistant_message: {
          id: 'm-2',
          message_id: 'm-2',
          session_id: 's-1',
          role: 'assistant',
          content: '分析完成',
          created_at: '2026-05-28T00:02:00.000Z',
        },
        session: {
          id: 's-1',
          session_id: 's-1',
          title: 'BTC',
          mode: 'simulated',
          created_at: '2026-05-28T00:00:00.000Z',
        },
        detail: {
          messages: [
            {
              id: 'm-1',
              message_id: 'm-1',
              session_id: 's-1',
              role: 'user',
              content: '看一下 BTC',
              created_at: '2026-05-28T00:01:00.000Z',
            },
            {
              id: 'm-2',
              message_id: 'm-2',
              session_id: 's-1',
              role: 'assistant',
              content: '分析完成',
              created_at: '2026-05-28T00:02:00.000Z',
            },
          ],
        },
      },
    })

    await expect(chat('s-1', '看一下 BTC')).resolves.toMatchObject({
      message: {
        id: 'm-2',
        message_id: 'm-2',
        session_id: 's-1',
        role: 'assistant',
        content: '分析完成',
      },
      messages: [
        { id: 'm-1', message_id: 'm-1', role: 'user' },
        { id: 'm-2', message_id: 'm-2', role: 'assistant' },
      ],
      session: {
        id: 's-1',
        session_id: 's-1',
      },
    })
  })

  it('订单草案、巡检状态和巡检配置保持单一 snake_case 请求响应', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            id: 'd-1',
            draft_id: 'd-1',
            session_id: 's-1',
            inst_id: 'ETH-USDT-SWAP',
            mode: 'simulated',
            side: 'sell',
            order_type: 'market',
            size: '0.2',
            price: '',
            status: 'confirmed',
            created_at: '2026-05-28T00:00:00.000Z',
            updated_at: '2026-05-28T00:00:00.000Z',
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          running: false,
          current_phase: 'idle',
          last_run_started_at: null,
          last_run_finished_at: null,
          last_run_summary: { candidate_count: 0 },
          last_error: '',
          recent_events: [],
          settings: {
            enabled: true,
            interval_seconds: 120,
            symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
          },
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          enabled: false,
          interval_seconds: 180,
          symbols: ['BTC-USDT-SWAP', 'SOL-USDT-SWAP'],
          scan_limit: 12,
          timeframes: ['15m', '1H'],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          settings: {
            enabled: false,
            interval_seconds: 240,
            symbols: ['BTC-USDT-SWAP'],
            scan_limit: 8,
            inst_type: 'SWAP',
            notification_cooldown_seconds: 60,
          },
          status: {
            running: false,
            current_phase: 'idle',
          },
        },
      })

    await expect(fetchOrderDrafts()).resolves.toEqual([
      expect.objectContaining({
        id: 'd-1',
        draft_id: 'd-1',
        inst_id: 'ETH-USDT-SWAP',
        side: 'sell',
        order_type: 'market',
        size: '0.2',
        status: 'confirmed',
        created_at: '2026-05-28T00:00:00.000Z',
      }),
    ])

    await expect(fetchPatrolStatus()).resolves.toMatchObject({
      running: false,
      current_phase: 'idle',
      last_run_started_at: null,
      settings: {
        enabled: true,
        interval_seconds: 120,
        symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
      },
    })

    await expect(fetchPatrolConfig()).resolves.toMatchObject({
      enabled: false,
      interval_seconds: 180,
      symbols: ['BTC-USDT-SWAP', 'SOL-USDT-SWAP'],
      scan_limit: 12,
      timeframes: ['15m', '1H'],
    })

    await expect(updatePatrolConfig({
      enabled: false,
      interval_seconds: 240,
      symbols: ['BTC-USDT-SWAP'],
      scan_limit: 8,
      inst_type: 'SWAP',
      notification_cooldown_seconds: 60,
    })).resolves.toMatchObject({
      settings: {
        enabled: false,
        interval_seconds: 240,
        symbols: ['BTC-USDT-SWAP'],
      },
      status: {
        running: false,
      },
    })

    const updateBody = (invokeMock.mock.calls[3]?.[1] as { req?: { body?: Record<string, unknown> } })?.req?.body
    expect(updateBody).toMatchObject({
      enabled: false,
      interval_seconds: 240,
      symbols: ['BTC-USDT-SWAP'],
      scan_limit: 8,
      inst_type: 'SWAP',
      notification_cooldown_seconds: 60,
    })
    expect(updateBody).not.toHaveProperty('intervalSeconds')
    expect(updateBody).not.toHaveProperty('scanLimit')
    expect(updateBody).not.toHaveProperty('instType')
  })

  it('手动巡检结果读取持久化后的 run 记录字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        id: 'patrol-1',
        run_id: 'patrol-1',
        mode: 'simulated',
        status: 'completed',
        summary: { candidate_count: 0 },
        candidates: [],
        result: {},
        event: {},
        settings: {},
        started_at: '2026-05-28T00:00:00.000Z',
        finished_at: '2026-05-28T00:01:00.000Z',
      },
    })

    await expect(runPatrolNow()).resolves.toMatchObject({
      run_id: 'patrol-1',
      status: 'completed',
      summary: { candidate_count: 0 },
      candidates: [],
      started_at: '2026-05-28T00:00:00.000Z',
      finished_at: '2026-05-28T00:01:00.000Z',
    })
  })

  it('列表接口不读取旧 wrapper 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({ tools: [{ name: 'legacy_tool', description: '旧工具' }] })
      .mockResolvedValueOnce({ sessions: [{ id: 'legacy-session' }] })
      .mockResolvedValueOnce({ order_drafts: [{ id: 'legacy-draft' }] })

    await expect(fetchAgentCapabilities()).resolves.toEqual([])
    await expect(fetchSessions()).resolves.toEqual([])
    await expect(fetchOrderDrafts()).resolves.toEqual([])
  })
})
