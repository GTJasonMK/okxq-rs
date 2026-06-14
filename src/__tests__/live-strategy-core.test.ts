import { describe, expect, it } from 'vitest'
import {
  equityHistory,
  equitySnapshot,
  liveOrder,
  status,
} from './fixtures/liveStrategy'
import {
  dailySummariesFromSnapshots,
  detailDataScopeText,
  liveRuntimeDataScope,
  runtimeRefreshNoticeText,
  scopedLiveEquityHistory,
  scopedLiveOrders,
} from '@/utils/liveStrategyCore'
import { settledErrorMessages } from '@/utils/settled'

describe('liveStrategyCore', () => {
  it('每日权益汇总用有限快照时间排序并回退创建时间', () => {
    const fallbackCreatedAt = Date.parse('2026-05-28T00:01:00.000Z')
    const latestTimestamp = Date.parse('2026-05-28T01:00:00.000Z')

    const [summary] = dailySummariesFromSnapshots([
      equitySnapshot({
        id: 1,
        timestamp: Number.POSITIVE_INFINITY,
        created_at: fallbackCreatedAt,
        equity: 1001,
        today_pnl: 1,
      }),
      equitySnapshot({
        id: 2,
        timestamp: latestTimestamp,
        created_at: latestTimestamp,
        equity: 1015,
        today_pnl: 15,
      }),
    ])

    expect(summary?.start_timestamp).toBe(fallbackCreatedAt)
    expect(summary?.end_timestamp).toBe(latestTimestamp)
    expect(summary?.first_equity).toBe(1001)
    expect(summary?.last_equity).toBe(1015)
    expect(summary?.today_pnl).toBe(15)
  })

  it('按运行模式和 run 过滤订单', () => {
    const scoped = scopedLiveOrders([
      liveOrder({ id: 1, mode: 'simulated', run_id: 'run-a' }),
      liveOrder({ id: 2, mode: 'simulated', run_id: 'run-b' }),
      liveOrder({ id: 3, mode: 'live', run_id: 'run-a' }),
    ], {
      mode: 'simulated',
      runId: 'run-a',
    })

    expect(scoped.map(order => order.id)).toEqual([1])
  })

  it('按运行范围过滤权益快照并重建每日汇总', () => {
    const scoped = scopedLiveEquityHistory(equityHistory({
      mode: 'simulated',
      run_id: 'run-a',
      snapshots: [
        equitySnapshot({ id: 1, mode: 'simulated', run_id: 'run-a', equity: 1000 }),
        equitySnapshot({ id: 2, mode: 'simulated', run_id: 'run-b', equity: 2000 }),
        equitySnapshot({ id: 3, mode: 'live', run_id: 'run-a', equity: 3000 }),
      ],
    }), {
      mode: 'simulated',
      runId: 'run-a',
    })

    expect(scoped?.count).toBe(1)
    expect(scoped?.snapshots.map(snapshot => snapshot.id)).toEqual([1])
    expect(scoped?.daily).toHaveLength(1)
    expect(scoped?.daily[0]?.last_equity).toBe(1000)
  })

  it('说明当前运行范围和隐藏数据', () => {
    const text = detailDataScopeText({
      status: status({ running: true, run_id: '123456789012345678901234' }),
      mode: 'simulated',
      runId: '123456789012345678901234',
      hiddenOrderCount: 2,
      hiddenEquityByScope: true,
      scopedEquityHistory: null,
    })

    expect(text).toContain('模拟盘')
    expect(text).toContain('当前运行 123456789012345678…')
    expect(text).toContain('已隐藏 2 条非当前范围历史记录')
    expect(text).toContain('权益不属于当前范围已隐藏')
  })

  it('运行刷新失败提示保留上次刷新状态', () => {
    expect(runtimeRefreshNoticeText(null, 0)).toBe('')
    expect(runtimeRefreshNoticeText('network down', 0)).toContain('尚无成功刷新')
    expect(runtimeRefreshNoticeText('network down', 0)).toContain('network down')
  })

  it('运行数据请求范围跟随当前状态并回退启动模式', () => {
    expect(liveRuntimeDataScope(null, 'simulated')).toEqual({ mode: 'simulated', runId: '' })
    expect(liveRuntimeDataScope(status({ mode: 'live', run_id: 'run-live' }), 'simulated')).toEqual({
      mode: 'live',
      runId: 'run-live',
    })
  })

  it('Promise settled 错误摘要只输出失败项', () => {
    const errors = settledErrorMessages([
      { label: '状态', result: { status: 'fulfilled', value: 'ok' } },
      { label: '订单', result: { status: 'rejected', reason: new Error('bad order') } },
    ], reason => reason instanceof Error ? reason.message : String(reason))

    expect(errors).toEqual(['订单: bad order'])
  })
})
