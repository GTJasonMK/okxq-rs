import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LiveDecisionDiagnosticsPanel from '@/components/live/LiveDecisionDiagnosticsPanel.vue'
import type { LiveDecisionActionSummary, LiveStrategyAction } from '@/types'
import { decisionDiagnostics } from './fixtures/liveStrategy'

function action(overrides: Partial<LiveStrategyAction>): LiveStrategyAction {
  return {
    action: 'hold',
    symbol: 'BTC-USDT-SWAP',
    side: '',
    order_type: 'market',
    price: null,
    reference_price: null,
    trigger_price: null,
    price_source: '',
    reason: '',
    strength: null,
    timestamp: 1_780_000_000_000,
    position_size: null,
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
    ...overrides,
  }
}

function summary(overrides: Partial<LiveDecisionActionSummary>): LiveDecisionActionSummary {
  return {
    open_position: 0,
    close_position: 0,
    place_risk_order: 0,
    cancel_order: 0,
    modify_order: 0,
    hold: 0,
    total: 0,
    ...overrides,
  }
}

describe('LiveDecisionDiagnosticsPanel', () => {
  it('显示撤单和改单动作的目标订单与变更字段', () => {
    const wrapper = mount(LiveDecisionDiagnosticsPanel, {
      props: {
        diagnostics: decisionDiagnostics({
          actions: [
            action({
              action: 'cancel_order',
              symbol: 'ETH-USDT-SWAP',
              reason: 'cancel stale stop',
              order_id: 'algo-order-1',
              client_order_id: 'algo-client-order-1',
              target_order_kind: 'algo',
              target_order_type: 'stop_loss_market',
            }),
            action({
              action: 'modify_order',
              symbol: 'BTC-USDT-SWAP',
              reason: 'move stop',
              client_order_id: 'exchange-client-order-1',
              new_size: '2',
              new_price: '94.5',
              request_id: 'modify-request-1',
              cancel_on_fail: true,
              target_order_kind: 'exchange',
            }),
          ],
          action_summary: summary({
            cancel_order: 1,
            modify_order: 1,
            total: 2,
          }),
        }),
        scopeText: 'BTC · 15m',
        loading: false,
        refreshSource: '',
        error: '',
        autoEnabled: true,
        running: true,
      },
    })

    expect(wrapper.text()).toContain('订单目标/变更')
    expect(wrapper.text()).toContain('撤单')
    expect(wrapper.text()).toContain('改单')
    expect(wrapper.text()).toContain('保护单')
    expect(wrapper.text()).toContain('普通订单')
    expect(wrapper.text()).toContain('algo-order-1')
    expect(wrapper.text()).toContain('客户单 exchange...rder-1')
    expect(wrapper.text()).toContain('新数量 2')
    expect(wrapper.text()).toContain('新价 94.5')
    expect(wrapper.text()).toContain('失败撤单')

    const managementCells = wrapper.findAll('.management-cell')
    expect(managementCells[0].attributes('title')).toContain('订单ID algo-order-1')
    expect(managementCells[0].attributes('title')).toContain('类型 止损市价')
    expect(managementCells[1].attributes('title')).toContain('客户订单ID exchange-client-order-1')
    expect(managementCells[1].attributes('title')).toContain('请求ID modify-request-1')

    wrapper.unmount()
  })

  it('显示被执行合约拒绝的动作和具体跳过原因', () => {
    const skipped = action({
      action: 'open_position',
      symbol: 'ETH-USDT-SWAP',
      side: 'long',
      order_type: 'limit',
      price: null,
      reason: 'open long',
      position_size: 0.2,
      raw: {
        _execution_skip_reason: '动作 open_position 的 order_type=limit 需要显式 price',
      },
    })
    const wrapper = mount(LiveDecisionDiagnosticsPanel, {
      props: {
        diagnostics: decisionDiagnostics({
          actions: [skipped],
          action_summary: summary({
            open_position: 1,
            total: 1,
          }),
          execution_decision: {
            verdict: 'blocked',
            summary: '执行链路被阻断：actions。',
            executable_intent_count: 0,
            risk_action_count: 0,
            skipped_action_count: 1,
            idle_action_count: 0,
            skipped_actions: [skipped],
            gates: [
              {
                key: 'actions',
                label: '策略动作',
                status: 'block',
                passed: false,
                blocking: true,
                detail: '动作合约未通过：动作 open_position 的 order_type=limit 需要显式 price',
              },
            ],
          },
        }),
        scopeText: 'BTC · 15m',
        loading: false,
        refreshSource: '',
        error: '',
        autoEnabled: true,
        running: true,
      },
    })

    expect(wrapper.text()).toContain('跳过 1 个动作')
    expect(wrapper.text()).toContain('执行意图 0')
    expect(wrapper.text()).toContain('已阻断')
    expect(wrapper.text()).toContain('动作合约未通过')
    expect(wrapper.text()).toContain('动作 open_position 的 order_type=limit 需要显式 price')

    wrapper.unmount()
  })
})
