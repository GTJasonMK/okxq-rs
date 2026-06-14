import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import BacktestResultCard from '@/components/backtest/BacktestResultCard.vue'
import type { BacktestResult } from '@/types'

describe('BacktestResultCard', () => {
  it('点击策略参数按钮后只读显示本次回测参数', async () => {
    const wrapper = mount(BacktestResultCard, {
      props: {
        result: backtestResult({
          params: {
            leverage: 3,
            strict_context_gating: false,
          },
        }),
      },
      global: {
        stubs: {
          Teleport: true,
        },
      },
    })

    await wrapper.get('.rc-strategy-param-btn').trigger('click')

    expect(wrapper.find('.param-modal').exists()).toBe(true)
    expect(wrapper.text()).toContain('回测参数')
    expect(wrapper.text()).toContain('ML Trade Selector Forward Candidate V1')
    expect(wrapper.text()).toContain('策略ID')
    expect(wrapper.text()).toContain('BTC-USDT-SWAP')
    expect(wrapper.text()).toContain('杠杆倍数')
    expect(wrapper.text()).toContain('leverage')
    expect(wrapper.text()).toContain('严格上下文门控')
    expect(wrapper.text()).toContain('strict_context_gating')
    expect(wrapper.text()).toContain('3x')
    expect(wrapper.text()).toContain('否')
    expect(wrapper.find('.param-editor-input').exists()).toBe(false)
    expect(wrapper.find('.param-submit-btn').exists()).toBe(false)

    await wrapper.get('.param-close').trigger('click')

    expect(wrapper.find('.param-modal').exists()).toBe(false)
  })

  it('点击引擎参数按钮后显示费用和执行模型', async () => {
    const wrapper = mount(BacktestResultCard, {
      props: {
        result: backtestResult({
          contract_mode: true,
          cost_model: {
            fee_rate: 0.0005,
            slippage_rate: 0.001,
            funding_rate_8h: 0.0003,
            total_funding: -1.25,
            leverage: 3,
            position_size: 0.2,
            position_size_mode: 'margin_fraction',
            allow_short: true,
          },
          execution_model: {
            timing: 'next_open',
            delay_bars: 1,
            price: 'open',
          },
          params: {
            commission_rate: 0.0007,
            funding_rate_8h: 0.0003,
            strict_context_gating: false,
          },
        }),
      },
      global: {
        stubs: {
          Teleport: true,
        },
      },
    })

    await wrapper.get('.rc-engine-param-btn').trigger('click')

    expect(wrapper.find('.param-modal').exists()).toBe(true)
    expect(wrapper.text()).toContain('回测引擎参数')
    expect(wrapper.text()).toContain('手续费率')
    expect(wrapper.text()).toContain('资金费率/8小时')
    expect(wrapper.text()).toContain('滑点率')
    expect(wrapper.text()).toContain('执行时机')
    expect(wrapper.text()).toContain('费用模型')
    expect(wrapper.text()).toContain('cost_model')
    expect(wrapper.text()).toContain('执行模型')
    expect(wrapper.text()).toContain('execution_model')
    expect(wrapper.text()).toContain('佣金/手续费率')
    expect(wrapper.text()).toContain('commission_rate')
    expect(wrapper.text()).toContain('0.000500 (0.0500%)')
    expect(wrapper.text()).toContain('下一根K线开盘成交')
    expect(wrapper.find('.param-editor-input').exists()).toBe(false)
    expect(wrapper.find('.param-submit-btn').exists()).toBe(false)
  })

  it('显示运行时动作回测的执行诊断', () => {
    const wrapper = mount(BacktestResultCard, {
      props: {
        result: backtestResult({
          runtime_action_summary: {
            open_action_count: 3,
            open_actions_with_planned_exit: 3,
            planned_exit_contract: 'planned_exit_complete',
            planned_close_count: 2,
            risk_close_count: 1,
            open_positions_missing_mark_count: 1,
            warnings: [
              'open_positions_missing_mark_price',
            ],
          },
        }),
      },
    })

    const diagnosticItems = wrapper.findAll('.diagnostic-item').map(item => item.text())

    expect(wrapper.find('.rc-diagnostics').exists()).toBe(true)
    expect(wrapper.text()).toContain('执行诊断')
    expect(wrapper.text()).toContain('计划退出完整')
    expect(diagnosticItems.some(item => item.includes('计划退出') && item.includes('3/3'))).toBe(true)
    expect(diagnosticItems.some(item => item.includes('计划平仓') && item.includes('2'))).toBe(true)
    expect(diagnosticItems.some(item => item.includes('止盈止损') && item.includes('1'))).toBe(true)
    expect(diagnosticItems.some(item => item.includes('缺标记价') && item.includes('1'))).toBe(true)
    expect(wrapper.text()).toContain('未平仓持仓缺少历史标记价')
  })

  it('普通回测结果不显示执行诊断', () => {
    const wrapper = mount(BacktestResultCard, {
      props: {
        result: backtestResult(),
      },
    })

    expect(wrapper.find('.rc-diagnostics').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('执行诊断')
  })

  it('缺少运行时摘要的旧结果显示结果检查', () => {
    const wrapper = mount(BacktestResultCard, {
      props: {
        result: backtestResult({
          backtest_result_integrity: {
            status: 'unverified',
            label: '旧结果待复核',
            issues: [
              'runtime_action_summary_missing',
              'runtime_open_actions_missing_planned_exit',
              'runtime_closes_without_planned_lifecycle',
            ],
          },
        }),
      },
    })

    expect(wrapper.find('.rc-integrity').exists()).toBe(true)
    expect(wrapper.text()).toContain('结果检查')
    expect(wrapper.text()).toContain('旧结果待复核')
    expect(wrapper.text()).toContain('缺少运行时摘要')
    expect(wrapper.text()).toContain('开仓动作缺少计划退出')
    expect(wrapper.text()).toContain('平仓只来自止盈止损或回测结束')
    expect(wrapper.find('.rc-diagnostics').exists()).toBe(false)
  })
})

function backtestResult(overrides: Partial<BacktestResult> = {}): BacktestResult {
  return {
    result_id: '1',
    strategy_id: 'ml_trade_selector_forward_candidate_v1',
    strategy_name: 'ML Trade Selector Forward Candidate V1',
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    days: 30,
    initial_capital: 1000,
    final_equity: 1010,
    total_return_pct: 1,
    sharpe_ratio: 0,
    max_drawdown_pct: 0,
    win_rate_pct: 0,
    total_trades: 0,
    winning_trades: 0,
    losing_trades: 0,
    profit_factor: 0,
    trades: [],
    orders: [],
    fills: [],
    rejected_orders: [],
    trade_events_total: 0,
    trades_truncated: false,
    candles: [],
    indicators: {},
    equity_curve: [],
    created_at: '2026-06-05T00:00:00.000Z',
    ...overrides,
  }
}
