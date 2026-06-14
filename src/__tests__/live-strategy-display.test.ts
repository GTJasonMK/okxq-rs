import { describe, expect, it } from 'vitest'
import {
  buildLiveStrategyKpis,
  decisionKpi,
  latestLiveEquitySnapshot,
  latestLiveOrder,
  orderStatusLabel,
} from '@/utils/liveStrategyDisplay'
import {
  decisionDiagnostics,
  equityHistory,
  equitySnapshot,
  liveOrder,
  status,
} from './fixtures/liveStrategy'
import {
  formatOrderAction,
  isFailedLiveOrder,
  liveOrderHistoryStatusClass,
} from '@/utils/liveStrategyDisplay/orders'

describe('liveStrategyDisplay', () => {
  it('最新权益快照按有效动作时间排序并回退创建时间', () => {
    const latest = latestLiveEquitySnapshot(equityHistory({
      snapshots: [
        equitySnapshot({
          id: 1,
          timestamp: Number.POSITIVE_INFINITY,
          created_at: Date.parse('2026-05-28T01:00:00.000Z'),
          equity: 1001,
        }),
        equitySnapshot({
          id: 2,
          timestamp: Date.parse('2026-05-28T00:30:00.000Z'),
          created_at: Date.parse('2026-05-28T00:30:00.000Z'),
          equity: 1015,
        }),
      ],
    }))

    expect(latest?.id).toBe(1)
    expect(latest?.equity).toBe(1001)
  })

  it('最新订单按动作时间、创建时间和 id 选择', () => {
    const latest = latestLiveOrder([
      liveOrder({ id: 1, timestamp: 1000, created_at: 3000 }),
      liveOrder({ id: 2, timestamp: 2000, created_at: 2000 }),
      liveOrder({ id: 3, timestamp: Number.NaN, created_at: 1500 }),
      liveOrder({ id: 4, timestamp: 2000, created_at: 2000 }),
    ])

    expect(latest?.id).toBe(4)
  })

  it('保护单生命周期状态显示为可读中文', () => {
    expect(orderStatusLabel('algo_submitted')).toBe('保护单已提交')
    expect(orderStatusLabel('algo_live')).toBe('保护单生效中')
    expect(orderStatusLabel('algo_modify_requested')).toBe('保护单改单已请求')
    expect(orderStatusLabel('algo_effective')).toBe('保护单已触发')
  })

  it('订单管理动作显示为撤单改单而不是开仓兜底', () => {
    expect(formatOrderAction(liveOrder({
      action: 'cancel_order',
      side: 'hold',
      status: 'cancel_requested',
      success: true,
    }))).toBe('撤单')
    expect(formatOrderAction(liveOrder({
      action: 'modify_order',
      side: 'hold',
      status: 'modify_requested',
      success: true,
    }))).toBe('改单')
  })

  it('提交结果待确认订单即使 success=false 也不显示为失败', () => {
    const submitUnknown = liveOrder({
      status: 'submit_unknown',
      success: false,
    })
    const algoSubmitUnknown = liveOrder({
      status: 'algo_submit_unknown',
      success: false,
    })

    expect(isFailedLiveOrder(submitUnknown)).toBe(false)
    expect(isFailedLiveOrder(algoSubmitUnknown)).toBe(false)
    expect(liveOrderHistoryStatusClass(submitUnknown)).toBe('pending')
    expect(liveOrderHistoryStatusClass(algoSubmitUnknown)).toBe('pending')
  })

  it('用诊断、最新权益和订单状态生成运行 KPI', () => {
    const kpis = buildLiveStrategyKpis({
      status: status({ running: true }),
      equityHistory: equityHistory({
        snapshots: [
          equitySnapshot({
            id: 1,
            equity: 1050,
            total_pnl: 50,
            total_pnl_pct: 5,
            today_pnl: 12,
            today_pnl_pct: 1.2,
          }),
        ],
      }),
      orders: [
        liveOrder({ id: 1, timestamp: 1000, status: 'filled', success: true }),
        liveOrder({ id: 2, timestamp: 2000, status: 'order_failed', success: false }),
      ],
      diagnostics: decisionDiagnostics({
        summary: '开仓动作可执行',
        actions: [{
          action: 'open_position',
          symbol: 'BTC-USDT-SWAP',
          side: 'buy',
          order_type: 'market',
          price: 100,
          reference_price: 100,
          trigger_price: null,
          price_source: 'latest_candle',
          reason: 'entry',
          strength: 1,
          timestamp: 2000,
          position_size: 0.25,
          exchange_size: '',
          order_side: '',
          close_side: '',
          planned_exit_time: null,
          planned_exit_reason: '',
          planned_exit_contract: '',
          order_id: '',
          client_order_id: '',
          new_size: '',
          new_price: '',
          request_id: '',
          cancel_on_fail: false,
          target_order_kind: '',
          target_order_type: '',
          source_index: null,
          source_time: null,
          feature_bar_time: null,
          entry_time: null,
          planned_hold_bars: null,
          hold_bars: null,
          layer_id: '',
          family: '',
          action_timeframe: '',
          candidate_source: '',
          candidate_entry_price: null,
          raw: {},
        }],
        action_summary: {
          open_position: 1,
          close_position: 0,
          place_risk_order: 0,
          cancel_order: 0,
          modify_order: 0,
          hold: 0,
          total: 1,
        },
        execution_decision: {
          verdict: 'ready',
          summary: '可以提交订单',
          executable_intent_count: 1,
          risk_action_count: 0,
          skipped_action_count: 0,
          idle_action_count: 0,
          skipped_actions: [],
          gates: [],
        },
      }),
      decisionDiagnosticsLoading: false,
      decisionDiagnosticsScopeText: 'BTC · 15m',
      autoDecisionDiagnosticsEnabled: true,
    })

    expect(kpis).toHaveLength(4)
    expect(kpis[0]).toMatchObject({ label: '策略决策', value: '可执行', kind: 'ready' })
    expect(kpis[1]).toMatchObject({ label: '权益/收益', value: '+5.00%', kind: 'positive' })
    expect(kpis[1]?.detail).toContain('权益 1,050')
    expect(kpis[2]).toMatchObject({ label: '今日收益', value: '+1.20%', detail: '+12', kind: 'positive' })
    expect(kpis[3]).toMatchObject({ label: '订单', value: '2 条', kind: 'negative' })
    expect(kpis[3]?.detail).toContain('失败 1')
  })

  it('OKX 账户权益快照不把不可得收益展示成 0 收益', () => {
    const kpis = buildLiveStrategyKpis({
      status: status({ running: true }),
      equityHistory: equityHistory({
        source: 'okx_account_balance',
        pnl_available: false,
        snapshots: [
          equitySnapshot({
            equity: 1234.56,
            unrealized_pnl: 10,
            total_pnl: 0,
            total_pnl_pct: 0,
            today_pnl: 0,
            today_pnl_pct: 0,
            pnl_available: false,
            source: 'okx_account_balance',
          }),
        ],
      }),
      orders: [],
      diagnostics: null,
      decisionDiagnosticsLoading: false,
      decisionDiagnosticsScopeText: 'BTC · 15m',
      autoDecisionDiagnosticsEnabled: false,
    })

    expect(kpis[1]).toMatchObject({
      label: '权益/收益',
      value: '1,234.56',
      kind: 'positive',
    })
    expect(kpis[1]?.detail).toContain('OKX 账户权益')
    expect(kpis[1]?.detail).toContain('+10')
    expect(kpis[2]).toMatchObject({
      label: '今日收益',
      value: '未提供',
      kind: 'neutral',
    })
  })

  it('没有 OKX 权益快照时不使用旧 paper 状态字段伪造权益收益', () => {
    const kpis = buildLiveStrategyKpis({
      status: status({ running: true }),
      equityHistory: null,
      orders: [],
      diagnostics: null,
      decisionDiagnosticsLoading: false,
      decisionDiagnosticsScopeText: 'BTC · 15m',
      autoDecisionDiagnosticsEnabled: false,
    })

    expect(kpis[1]).toMatchObject({
      label: '权益/收益',
      value: '暂无',
      kind: 'neutral',
    })
    expect(kpis[1]?.detail).toContain('等待 OKX 账户权益')
    expect(kpis[1]?.detail).not.toContain('1,234.56')
    expect(kpis[1]?.detail).not.toContain('+99')
    expect(kpis[2]).toMatchObject({
      label: '今日收益',
      value: '未提供',
      kind: 'neutral',
    })
  })

  it('没有动作时展示等待决策', () => {
    const kpi = decisionKpi({
      diagnostics: decisionDiagnostics(),
      decisionDiagnosticsLoading: false,
      decisionDiagnosticsScopeText: 'BTC · 15m',
      autoDecisionDiagnosticsEnabled: false,
    })

    expect(kpi).toMatchObject({
      label: '策略决策',
      value: '等待',
      kind: 'neutral',
    })
    expect(kpi.detail).toContain('策略当前未返回动作')
  })

  it('订单管理和保护单动作也按可执行动作展示', () => {
    const kpi = decisionKpi({
      diagnostics: decisionDiagnostics({
        action_summary: {
          open_position: 0,
          close_position: 0,
          place_risk_order: 1,
          cancel_order: 1,
          modify_order: 1,
          hold: 0,
          total: 3,
        },
        execution_decision: {
          verdict: 'preview',
          summary: '存在订单管理动作',
          executable_intent_count: 3,
          risk_action_count: 1,
          skipped_action_count: 0,
          idle_action_count: 0,
          skipped_actions: [],
          gates: [],
        },
      }),
      decisionDiagnosticsLoading: false,
      decisionDiagnosticsScopeText: 'BTC · 15m',
      autoDecisionDiagnosticsEnabled: true,
    })

    expect(kpi).toMatchObject({
      label: '策略决策',
      value: '3 个动作',
      kind: 'ready',
    })
    expect(kpi.detail).toContain('保护单 1')
    expect(kpi.detail).toContain('撤单 1')
    expect(kpi.detail).toContain('改单 1')
  })

})
