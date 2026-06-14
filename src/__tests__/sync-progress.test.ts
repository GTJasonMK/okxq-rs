import { describe, expect, it } from 'vitest'
import { parseDateTimeMs, summarizeSyncProgress } from '@/utils/syncProgress'
import type { SyncJob } from '@/types'

describe('同步进度工具', () => {
  it('解析 Rust 纳秒精度 RFC3339 时间戳', () => {
    const parsed = parseDateTimeMs('2026-05-22T02:08:12.075608200+00:00')

    expect(parsed).toBe(Date.UTC(2026, 4, 22, 2, 8, 12, 75))
  })

  it('解析空格分隔的数据库时间戳', () => {
    const parsed = parseDateTimeMs('2026-05-23 08:41:10')

    expect(parsed).toBe(Date.UTC(2026, 4, 23, 8, 41, 10))
  })

  it('失败终态保留后端进度，避免进度条看起来消失', () => {
    const summary = summarizeSyncProgress([syncJob({
      status: 'failed',
      progress: 100,
      error: 'runtime error: OKX API error 51000: Parameter bar error',
    })])

    expect(summary.statusLabel).toBe('同步失败')
    expect(summary.progress).toBe(100)
    expect(summary.segments).toHaveLength(1)
    expect(summary.segments[0].progress).toBe(100)
    expect(summary.primaryText).toBe('OKX 周期参数错误')
  })

  it('异常数值不会把同步进度展示成 NaN', () => {
    const summary = summarizeSyncProgress([syncJob({
      progress: Number.NaN,
      fetched_count: Number.POSITIVE_INFINITY,
      target_fetch_count: Number.NaN,
      saved_count: 'not-a-number' as unknown as number,
      target_save_count: Number.NEGATIVE_INFINITY,
      batches: Number.NaN,
      target_batches: Number.POSITIVE_INFINITY,
      api_calls: Number.NaN,
    })])

    expect(summary.progress).toBe(0)
    expect(summary.fetched).toBe(0)
    expect(summary.targetFetch).toBe(0)
    expect(summary.saved).toBe(0)
    expect(summary.targetSave).toBe(0)
    expect(summary.batches).toBe(0)
    expect(summary.targetBatches).toBe(0)
    expect(summary.apiCalls).toBe(0)
    expect(summary.taskText).toBe('任务 0 / 1 · 运行 1')
    expect(summary.segments[0]).toMatchObject({
      done: 0,
      total: 100,
      progress: 0,
      text: '0%',
    })
  })

  it('字符串数字不会被同步进度工具当作有效后端计数', () => {
    const summary = summarizeSyncProgress([syncJob({
      progress: '50' as unknown as number,
      fetched_count: '12' as unknown as number,
      target_fetch_count: '20' as unknown as number,
      saved_count: '8' as unknown as number,
      target_save_count: '20' as unknown as number,
    })])

    expect(summary.progress).toBe(0)
    expect(summary.fetched).toBe(0)
    expect(summary.targetFetch).toBe(0)
    expect(summary.saved).toBe(0)
    expect(summary.targetSave).toBe(0)
    expect(summary.primaryText).toBe('同步中')
    expect(summary.segments[0]).toMatchObject({
      done: 0,
      total: 100,
      progress: 0,
      text: '0%',
    })
  })

  it('运行中优先展示当前阶段处理量，不再展示聚合预计数', () => {
    const summary = summarizeSyncProgress([syncJob({
      progress: 88,
      message: '落库 3m 中：已处理 2,000 / 172,644 条对齐 K 线，实际写入 1 条',
      derived_count: 1,
      target_derive_count: 319_541,
    })])

    expect(summary.phaseLabel).toBe('正在对齐')
    expect(summary.primaryText).toBe('落库 3m 中：已处理 2,000 / 172,644 条对齐 K 线，实际写入 1 条')
    expect(summary.segments).toHaveLength(1)
    expect(summary.secondaryText).toBe('')
  })

  it('完成态不再展示拉取落库对齐的内部预计总量', () => {
    const summary = summarizeSyncProgress([syncJob({
      status: 'completed',
      progress: 100,
      fetched_count: 5,
      target_fetch_count: 5,
      saved_count: 5,
      target_save_count: 5,
      derived_count: 10,
      target_derive_count: 319_541,
    })])

    expect(summary.statusLabel).toBe('同步完成')
    expect(summary.primaryText).toBe('任务完成')
    expect(summary.segments).toHaveLength(1)
    expect(summary.segments[0]).toMatchObject({
      label: '完成',
      text: '100%',
    })
  })
})

function syncJob(overrides: Partial<SyncJob> = {}): SyncJob {
  return {
    task_id: 'sync_test',
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    source_timeframe: '1m',
    target_timeframes: ['1H'],
    mode: 'window',
    status: 'running',
    progress: 0,
    created_at: '2026-05-23T08:00:00.000000000+00:00',
    updated_at: '2026-05-23T08:00:00.000000000+00:00',
    message: '',
    error: '',
    fetched_count: 0,
    target_fetch_count: 0,
    saved_count: 0,
    target_save_count: 0,
    inserted_count: 0,
    derived_count: 0,
    target_derive_count: 0,
    batches: 0,
    target_batches: 0,
    api_calls: 0,
    candle_count: 0,
    history_complete: false,
    ...overrides,
  }
}
