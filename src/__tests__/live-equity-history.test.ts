import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import LiveEquityHistory from '@/components/live/LiveEquityHistory.vue'
import type {
  LiveEquityDailySummary,
  LiveEquityHistory as LiveEquityHistoryData,
  LiveEquitySnapshot,
} from '@/types'

describe('LiveEquityHistory', () => {
  it('用策略权益语义显示模式和最新快照时间', () => {
    const latestTimestamp = Date.parse('2026-05-28T16:01:00.000Z')
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          mode: 'live',
          snapshots: [
            snapshot({ timestamp: Date.parse('2026-05-28T15:00:00.000Z'), equity: 1005 }),
            snapshot({ timestamp: latestTimestamp, equity: 1012.5 }),
          ],
          count: 2,
        }),
      },
    })

    expect(wrapper.find('.le-title').text()).toBe('策略权益（实盘模式）')
    expect(wrapper.find('.le-run').text()).toContain('实盘模式')
    expect(wrapper.find('.le-run').text()).toContain(`最新 ${formatDateTime(latestTimestamp)}`)
    expect(wrapper.text()).toContain('1.01K')
    expect(wrapper.text()).not.toContain('模拟权益')
    expect(wrapper.find('.le-summary').exists()).toBe(false)
    expect(wrapper.find('.le-visuals').exists()).toBe(false)
    expect(wrapper.find('.le-line-chart').exists()).toBe(false)
    expect(wrapper.find('.le-bar-chart').exists()).toBe(false)
  })

  it('无快照时显示明确的策略权益空状态', () => {
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history(),
      },
    })

    expect(wrapper.find('.le-title').text()).toBe('策略权益（模拟模式）')
    expect(wrapper.find('.le-run').text()).toContain('暂无快照')
    const emptyStates = wrapper.findAll('.empty-state')
    expect(emptyStates).toHaveLength(2)
    expect(emptyStates[0]?.text()).toContain('暂无按天权益汇总')
    expect(emptyStates[0]?.text()).toContain('同步 OKX 账户权益')
    expect(emptyStates[1]?.text()).toContain('暂无最近权益快照')
    expect(emptyStates[1]?.text()).toContain('同步 OKX 账户权益')
  })

  it('OKX balance 快照按账户权益展示且不伪造价格和持仓', () => {
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          run_id: '',
          source: 'okx_account_balance',
          pnl_available: false,
          snapshots: [
            snapshot({
              run_id: '',
              equity: 1234.56,
              price: 0,
              position_side: 'multi',
              unrealized_pnl: 10,
              total_pnl: 0,
              today_pnl: 0,
              pnl_available: false,
              source: 'okx_account_balance',
            }),
          ],
          daily: [
            dailySummary({
              last_equity: 1234.56,
              total_pnl: 0,
              today_pnl: 0,
              pnl_available: false,
            }),
          ],
          count: 1,
        }),
      },
    })

    expect(wrapper.find('.le-title').text()).toBe('OKX账户权益（模拟盘）')
    expect(wrapper.find('.le-run').text()).toContain('OKX账户 · 模拟盘')
    expect(wrapper.text()).toContain('1.23K')
    expect(wrapper.find('.le-wrap table')?.text()).toContain('--')
    expect(wrapper.text()).not.toContain('+0.00%')
    expect(wrapper.text()).not.toContain('0.00000')
    expect(wrapper.text()).not.toContain('组合')
    expect(wrapper.findAll('th').map(item => item.text())).not.toContain('价格')
    expect(wrapper.findAll('th').map(item => item.text())).not.toContain('持仓')
    expect(wrapper.findAll('th').map(item => item.text())).toContain('账户权益')

    const dailyCells = wrapper.findAll('.le-section').at(0)?.find('tbody tr').findAll('td') ?? []
    expect(dailyCells[2]?.text()).toBe('--')
    expect(dailyCells[3]?.text()).toBe('--')
    expect(dailyCells[2]?.find('.pct').exists()).toBe(false)
    expect(dailyCells[3]?.find('.pct').exists()).toBe(false)
  })

  it('无历史数据时仍按外部运行模式展示标题', () => {
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: null,
        mode: 'live',
      },
    })

    expect(wrapper.find('.le-title').text()).toBe('策略权益（实盘模式）')
    expect(wrapper.find('.le-run').text()).toContain('暂无快照')
  })

  it('权益详情不依赖后端顺序，最新快照和最近日期优先展示', () => {
    const oldTimestamp = Date.parse('2026-05-27T16:00:00.000Z')
    const latestTimestamp = Date.parse('2026-05-28T16:00:00.000Z')
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          snapshots: [
            snapshot({ id: 1, timestamp: latestTimestamp, equity: 1010, trading_day: '2026-05-29' }),
            snapshot({ id: 2, timestamp: oldTimestamp, equity: 990, trading_day: '2026-05-28' }),
          ],
          daily: [
            dailySummary({ trading_day: '2026-05-28', end_timestamp: oldTimestamp, last_equity: 990 }),
            dailySummary({ trading_day: '2026-05-29', end_timestamp: latestTimestamp, last_equity: 1010 }),
          ],
          count: 2,
        }),
      },
    })

    expect(wrapper.find('.le-run').text()).toContain(`最新 ${formatDateTime(latestTimestamp)}`)
    const dailyRows = wrapper.findAll('.le-section').at(0)?.findAll('tbody tr') ?? []
    expect(dailyRows[0]?.text()).toContain('2026-05-29')
    const snapshotRows = wrapper.findAll('.le-section').at(1)?.findAll('tbody tr') ?? []
    expect(snapshotRows[0]?.text()).toContain(formatDateTime(latestTimestamp))
    expect(snapshotRows[0]?.text()).toContain('1.01K')
  })

  it('无效快照时间回退创建时间并参与最新优先排序', () => {
    const fallbackCreatedAt = Date.parse('2026-05-28T00:01:00.000Z')
    const latestTimestamp = Date.parse('2026-05-28T01:00:00.000Z')
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          snapshots: [
            snapshot({
              id: 1,
              timestamp: Number.POSITIVE_INFINITY,
              created_at: fallbackCreatedAt,
              equity: 1001,
            }),
            snapshot({
              id: 2,
              timestamp: latestTimestamp,
              created_at: latestTimestamp,
              equity: 1015,
            }),
          ],
          count: 2,
        }),
      },
    })

    expect(wrapper.find('.le-run').text()).toContain(`最新 ${formatDateTime(latestTimestamp)}`)
    const snapshotRows = wrapper.findAll('.le-section').at(1)?.findAll('tbody tr') ?? []
    expect(snapshotRows[0]?.text()).toContain(formatDateTime(latestTimestamp))
    expect(snapshotRows[1]?.text()).toContain(formatDateTime(fallbackCreatedAt))
  })

  it('把 multi 持仓快照展示为组合持仓', () => {
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          snapshots: [
            snapshot({
              position_side: 'multi',
              entry_price: 128.5,
              quantity: 3.25,
              equity: 1012,
            }),
          ],
          count: 1,
        }),
      },
    })

    const sideBadge = wrapper.find('.side-badge')
    expect(sideBadge.text()).toBe('组合')
    expect(sideBadge.classes()).toContain('portfolio')
  })

  it('明细表忽略无效快照权益并避免重复渲染图表', () => {
    const wrapper = mount(LiveEquityHistory, {
      props: {
        history: history({
          snapshots: [
            snapshot({ id: 1, timestamp: Date.parse('2026-05-27T00:00:00.000Z'), equity: 1000 }),
            snapshot({ id: 2, timestamp: Date.parse('2026-05-27T12:00:00.000Z'), equity: Number.NaN }),
            snapshot({ id: 3, timestamp: Date.parse('2026-05-28T00:00:00.000Z'), equity: 1010 }),
          ],
          count: 3,
        }),
      },
    })

    expect(wrapper.find('.le-line-chart').exists()).toBe(false)
    expect(wrapper.find('.le-bar-chart').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('NaN')
  })

})

function history(overrides: Partial<LiveEquityHistoryData> = {}): LiveEquityHistoryData {
  return {
    run_id: 'run-equity',
    mode: 'simulated',
    count: 0,
    snapshots: [],
    daily: [],
    ...overrides,
  }
}

function snapshot(overrides: Partial<LiveEquitySnapshot> = {}): LiveEquitySnapshot {
  return {
    id: 1,
    run_id: 'run-equity',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    symbol: 'BTC-USDT-SWAP',
    inst_id: 'BTC-USDT-SWAP',
    timeframe: '15m',
    inst_type: 'SWAP',
    mode: 'simulated',
    timestamp: Date.parse('2026-05-28T16:00:00.000Z'),
    time: '2026-05-29 00:00:00',
    trading_day: '2026-05-29',
    price: 100,
    position_side: 'flat',
    entry_price: 0,
    quantity: 0,
    initial_capital: 1000,
    day_start_equity: 1000,
    equity: 1000,
    realized_pnl: 0,
    unrealized_pnl: 0,
    total_pnl: 0,
    total_pnl_pct: 0,
    today_pnl: 0,
    today_pnl_pct: 0,
    created_at: Date.parse('2026-05-28T16:00:00.000Z'),
    ...overrides,
  }
}

function dailySummary(overrides: Partial<LiveEquityDailySummary> = {}): LiveEquityDailySummary {
  return {
    trading_day: '2026-05-29',
    start_timestamp: Date.parse('2026-05-28T16:00:00.000Z'),
    end_timestamp: Date.parse('2026-05-28T16:00:00.000Z'),
    start_time: '2026-05-29 00:00:00',
    end_time: '2026-05-29 00:00:00',
    snapshot_count: 1,
    first_equity: 1000,
    last_equity: 1000,
    day_start_equity: 1000,
    today_pnl: 0,
    today_pnl_pct: 0,
    total_pnl: 0,
    total_pnl_pct: 0,
    realized_pnl: 0,
    unrealized_pnl: 0,
    ...overrides,
  }
}

function formatDateTime(timestamp: number): string {
  return new Date(timestamp).toLocaleString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
