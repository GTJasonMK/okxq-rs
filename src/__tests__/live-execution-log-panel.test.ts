import { nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LiveExecutionLogPanel from '@/components/live/LiveExecutionLogPanel.vue'
import type { LiveExecutionLogEntry } from '@/types'

const ROW_HEIGHT = 33

describe('LiveExecutionLogPanel', () => {
  it('执行日志只挂载视窗附近行，避免 1 秒轮询时全量 DOM 重渲染', () => {
    const logs = perfLogs(160)

    const wrapper = mount(LiveExecutionLogPanel, {
      props: { logs },
    })

    expect(wrapper.findAll('.vl-log-row').length).toBeLessThanOrEqual(20)
    expect(wrapper.find('.vl-log-row strong').text()).toBe('log-160')

    wrapper.unmount()
  })

  it('执行日志滚动时切换可见窗口并保持最新优先顺序', async () => {
    const logs = perfLogs(160)
    const wrapper = mount(LiveExecutionLogPanel, {
      props: { logs },
    })
    const viewport = wrapper.find('.vl-log-list')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = ROW_HEIGHT * 80
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('log-080')
    expect(wrapper.findAll('.vl-log-row').length).toBeLessThanOrEqual(14)

    wrapper.unmount()
  })

  it('计划退出与同步阶段显示中文标签', () => {
    const wrapper = mount(LiveExecutionLogPanel, {
      props: {
        logs: [
          logEntry(0, 'strategy_audit', '策略审计存在警告'),
          logEntry(1, 'planned_exit_worker', '计划退出 worker 已启动'),
          logEntry(2, 'planned_exit', '计划退出到期，准备平仓'),
          logEntry(3, 'order_sync', '订单状态已更新'),
          logEntry(4, 'fill_sync', '成交同步完成'),
        ],
      },
    })

    expect(wrapper.text()).toContain('策略审计')
    expect(wrapper.text()).toContain('退出调度')
    expect(wrapper.text()).toContain('计划退出')
    expect(wrapper.text()).toContain('订单同步')
    expect(wrapper.text()).toContain('成交同步')
    expect(wrapper.text()).not.toContain('planned_exit_worker')
    expect(wrapper.text()).not.toContain('strategy_audit')

    wrapper.unmount()
  })

  it('跨运行持久化日志允许重复 seq 并同时显示', () => {
    const wrapper = mount(LiveExecutionLogPanel, {
      props: {
        logs: [
          { ...logEntry(1, 'start', 'run-a started'), run_id: 'run-a' },
          { ...logEntry(1, 'start', 'run-b started'), run_id: 'run-b' },
        ],
      },
    })

    expect(wrapper.findAll('.vl-log-row strong').map(row => row.text())).toEqual([
      'run-b started',
      'run-a started',
    ])

    wrapper.unmount()
  })

  it('执行日志直接显示策略内部进度', () => {
    const wrapper = mount(LiveExecutionLogPanel, {
      props: {
        logs: [
          {
            ...logEntry(1, 'candidate_generation', '生成候选中'),
            details: {
              event: 'strategy_log',
              details: {
                progress: 0.42,
                source: 'ml_trade_selector_runtime',
              },
            },
          },
        ],
      },
    })

    expect(wrapper.find('.vl-log-progress').text()).toBe('42%')
    expect(wrapper.text()).toContain('候选生成')
    expect(wrapper.text()).toContain('生成候选中')

    wrapper.unmount()
  })
})

function perfLogs(count: number): LiveExecutionLogEntry[] {
  return Array.from({ length: count }, (_, index) => {
    const seq = index + 1
    return {
      seq,
      run_id: 'run-live',
      mode: 'simulated',
      strategy_id: 'strategy-a',
      strategy_name: 'Strategy A',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      timestamp_ms: Date.parse('2026-06-06T00:00:00.000Z') + seq * 1000,
      time: '2026-06-06T00:00:00.000Z',
      stage: seq % 5 === 0 ? 'decision' : 'strategy',
      level: seq % 11 === 0 ? 'warn' : seq % 17 === 0 ? 'error' : 'info',
      message: `log-${String(seq).padStart(3, '0')}`,
      details: largeDetails(seq),
    }
  })
}

function logEntry(seq: number, stage: string, message: string): LiveExecutionLogEntry {
  return {
    seq,
    run_id: 'run-live',
    mode: 'simulated',
    strategy_id: 'strategy-a',
    strategy_name: 'Strategy A',
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    timestamp_ms: Date.parse('2026-06-06T00:00:00.000Z') + seq * 1000,
    time: '2026-06-06T00:00:00.000Z',
    stage,
    level: 'info',
    message,
    details: {},
  }
}

function largeDetails(seq: number): Record<string, unknown> {
  return Object.fromEntries(
    Array.from({ length: 18 }, (_, index) => [
      `metric_${index}`,
      {
        seq,
        value: seq * (index + 1),
        label: `detail-${seq}-${index}`,
      },
    ]),
  )
}
