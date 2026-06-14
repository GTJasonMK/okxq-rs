import { describe, expect, it } from 'vitest'
import {
  activeSyncJobs,
  buildPlanRows,
  buildPlanRowsFromScopeRecords,
  buildInventoryTableTotals,
  buildSyncRecordScopeIndex,
  buildWatchedRowSources,
  buildWatchedRows,
  buildWatchedRowsFromSources,
  createWatchedRowSourcesBuilder,
  createWatchedRowsBuilder,
  canOpenWatchRuleDialog,
  canSubmitWatchRuleDialog,
  countEnabledInstruments,
  dataCenterTabDescription,
  defaultWatchRuleForm,
  effectivePlansForRow,
  enabledSyncPlansFromGuardian,
  gapRepairMethodLabel,
  guardianCurrentTargetText,
  guardianErrorMessages,
  guardianPlanToSyncPlan,
  hasValidInventoryGapRange,
  inventoryGapKey,
  inventoryMarketSummary,
  inventoryRowsToSyncRecords,
  inventoryTimeframeCoverageLabel,
  inventoryTimeframeGapLabel,
  inventoryTimeframeRangeLabel,
  managedPlanLabelText,
  mergeSyncJobs,
  normalizeGuardianStatus,
  normalizeInventoryPayload,
  normalizeTickCollectorStatus,
  repairWatchedSymbolMessage,
  replaceSyncRecordScopes,
  replaceSyncRecordScopesInPlace,
  rowPlanSummary,
  sameSyncRuntimeSettings,
  storageCountLabel,
  syncRecordScopeKey,
  syncTaskSubmissionSummary,
  syncRecordsForScope,
  visibleSyncJobs,
  watchRuleFormFromRow,
  watchRuleSavedAction,
  watchRuleSubmitButtonLabel,
} from '@/utils/dataCenter'
import { summarizeSyncProgress } from '@/utils/syncProgress'
import { applyUnifiedSyncDays, normalizeSyncDays, normalizeSyncPlan } from '@/utils/syncPlans'
import type { SyncJob, SyncRecord, WatchedSymbol, WatchedSymbolSyncPlan } from '@/types'
import type { GuardianPlan, InventoryRow, WatchedRow } from '@/types/dataCenter'

describe('数据中心行级任务汇总', () => {
  it('派生模式旧规则缺少1m时自动补入1m底座', () => {
    const row = watchedSymbol({
      sync_days: 90,
      sync_plans: [syncPlan('1H', 90)],
    })

    const plans = effectivePlansForRow(row, [syncPlan('5m', 90)])

    expect(plans.map(plan => plan.timeframe)).toEqual(['1m', '1H'])
    expect(plans[0]).toMatchObject({
      timeframe: '1m',
      enabled: true,
      bootstrap_days: 90,
      archive_mode: 'rolling',
    })
    expect(rowPlanSummary(row, [])).toBe('1m/1H · 90天 · 1m底座派生')
  })

  it('关注页周期状态以数据库库存记录为准', () => {
    const records = inventoryRowsToSyncRecords([inventoryRow({
      timeframes: ['1m', '5m'],
    })])
    const [row] = buildWatchedRows(
      [watchedSymbol({
        sync_plans: [syncPlan('1H', 90)],
      })],
      [],
      records,
      [],
    )
    const planRows = buildPlanRows(row, 'SWAP', records, [])

    expect(records.map(record => record.timeframe)).toEqual(['1m', '5m'])
    expect(planRows.map(plan => [plan.timeframe, plan.policyLabel, plan.label])).toEqual([
      ['1m', '30天', '05/01 08:00 至 05/02 08:00 · 无缺失'],
      ['5m', '库内', '05/01 08:00 至 05/02 08:00 · 缺失 2'],
      ['1H', '90天', '未落库'],
    ])
  })

  it('计划行可复用按标的索引的库存记录且不串入其他标的', () => {
    const records = [
      syncRecord({
        inst_id: 'BTC-USDT-SWAP',
        timeframe: '1H',
        candle_count: 8760,
      }),
      syncRecord({
        inst_id: 'ETH-USDT-SWAP',
        timeframe: '5m',
        candle_count: 999,
      }),
    ]
    const [row] = buildWatchedRows([watchedSymbol()], [], records, [])
    const index = buildSyncRecordScopeIndex(records)
    const scopedRows = buildPlanRowsFromScopeRecords(
      row,
      'SWAP',
      syncRecordsForScope(index, row.swap_inst_id, 'SWAP'),
      [],
    )

    expect(scopedRows).toEqual(buildPlanRows(row, 'SWAP', records, []))
    expect(scopedRows.map(plan => plan.timeframe)).toEqual(['1m', '1H'])
    expect(scopedRows.some(plan => plan.timeframe === '5m')).toBe(false)
  })

  it('关注页包含数据库已有但未关注的标的', () => {
    const inventory = inventoryRow({
      symbol: 'ETH-USDT',
      base_ccy: 'ETH',
      inst_id: 'ETH-USDT-SWAP',
      watched: false,
      managed: false,
      timeframes: ['1m', '3m'],
    })
    const records = inventoryRowsToSyncRecords([inventory])
    const rows = buildWatchedRows(
      [watchedSymbol()],
      [],
      records,
      [],
      [inventory],
    )
    const inventoryOnly = rows.find(row => row.symbol === 'ETH-USDT')

    expect(rows.map(row => row.symbol)).toEqual(['BTC-USDT', 'ETH-USDT'])
    expect(inventoryOnly).toMatchObject({
      inventory_only: true,
      sync_spot: false,
      sync_swap: true,
      inventory_timeframes: ['1m', '3m'],
    })
    expect(rowPlanSummary(inventoryOnly!, [])).toBe('1m/3m · 库内数据 · 未接管规则')
    expect(buildPlanRows(inventoryOnly!, 'SWAP', records, []).map(plan => [
      plan.timeframe,
      plan.policyLabel,
      plan.label,
    ])).toEqual([
      ['1m', '库内', '05/01 08:00 至 05/02 08:00 · 无缺失'],
      ['3m', '库内', '05/01 08:00 至 05/02 08:00 · 无缺失'],
    ])
  })

  it('关注规则优先于同名库存标的，不重复显示', () => {
    const inventory = inventoryRow({
      symbol: 'BTC-USDT',
      timeframes: ['1m', '3m'],
    })
    const rows = buildWatchedRows(
      [watchedSymbol()],
      [],
      inventoryRowsToSyncRecords([inventory]),
      [],
      [inventory],
    )

    expect(rows).toHaveLength(1)
    expect(rows[0].inventory_only).toBeUndefined()
  })

  it('统一同步天数不会把全量计划降级为滚动窗口', () => {
    const plans = applyUnifiedSyncDays([
      syncPlan('1H', 90),
      syncPlan('1D', 365, 'full'),
    ], 120)

    expect(plans.find(plan => plan.timeframe === '1H')).toMatchObject({
      bootstrap_days: 120,
      archive_mode: 'rolling',
    })
    expect(plans.find(plan => plan.timeframe === '1D')).toMatchObject({
      bootstrap_days: 120,
      archive_mode: 'full',
    })
  })

  it('Guardian 计划天数按同步计划边界归一化', () => {
    expect(guardianPlanToSyncPlan({
      timeframe: '1H',
      enabled: true,
      bootstrap_days: 0,
      archive_mode: 'rolling',
    })).toMatchObject({ bootstrap_days: 1 })
    expect(guardianPlanToSyncPlan({
      timeframe: '1H',
      enabled: true,
      bootstrap_days: -7,
      archive_mode: 'rolling',
    })).toMatchObject({ bootstrap_days: 1 })
    expect(guardianPlanToSyncPlan({
      timeframe: '1H',
      enabled: true,
      bootstrap_days: 5000,
      archive_mode: 'rolling',
    })).toMatchObject({ bootstrap_days: 3650 })
    expect(guardianPlanToSyncPlan({
      timeframe: '1H',
      enabled: true,
      bootstrap_days: undefined,
      archive_mode: 'rolling',
    } as unknown as GuardianPlan)).toMatchObject({ bootstrap_days: 90 })
  })

  it('数据中心归一化不会把字符串 false 误判为 true', () => {
    const inventory = normalizeInventoryPayload({
      rows: [{
        symbol: 'eth-usdt',
        managed: 'false',
        watched: '0',
        orphan: 'false',
        candle_count: '10',
        timeframe_record_count: '2',
        markets: {
          SWAP: {
            inst_id: 'ETH-USDT-SWAP',
            inst_type: 'SWAP',
            managed: 'false',
            watched: '0',
          },
        },
      }],
    })

    expect(inventory.rows[0]).toMatchObject({
      symbol: 'ETH-USDT',
      managed: false,
      watched: false,
      orphan: false,
      candle_count: 0,
      timeframe_record_count: 0,
    })
    expect(inventory.rows[0].markets.SWAP).toMatchObject({
      managed: false,
      watched: false,
    })
    expect(normalizeTickCollectorStatus({ running: 'false' }).running).toBe(false)
    expect(normalizeGuardianStatus({ enabled: 'false', active: '0' })).toMatchObject({
      enabled: false,
      active: false,
    })
    expect(guardianPlanToSyncPlan({
      timeframe: '1H',
      enabled: 'false',
      bootstrap_days: 90,
      archive_mode: 'rolling',
    } as unknown as GuardianPlan)).toMatchObject({ enabled: false })
    expect(normalizeSyncPlan({
      timeframe: '1H',
      enabled: 'false',
      bootstrap_days: 90,
      archive_mode: 'rolling',
    } as unknown as WatchedSymbolSyncPlan)).toMatchObject({ enabled: false })
    expect(normalizeSyncPlan({
      timeframe: '1H',
      enabled: 'true',
      bootstrap_days: 90,
      archive_mode: 'rolling',
    } as unknown as WatchedSymbolSyncPlan)).toMatchObject({ enabled: false })
    expect(normalizeSyncPlan({
      timeframe: '1H',
      enabled: true,
      bootstrap_days: '30',
      archive_mode: 'rolling',
    } as unknown as WatchedSymbolSyncPlan)).toMatchObject({ bootstrap_days: 120 })
    expect(normalizeSyncDays('30')).toBe(90)
  })

  it('库存归一化只读取直接 rows，不读取旧 wrapper', () => {
    const inventory = normalizeInventoryPayload({
      summary: {
        symbol_count: 1,
      },
      inventory: {
        rows: [{
          symbol: 'BTC-USDT',
          managed: true,
        }],
      },
      data: {
        rows: [{
          symbol: 'ETH-USDT',
          managed: true,
        }],
      },
      rows: [],
    })

    expect(inventory.summary.symbol_count).toBe(1)
    expect(inventory.rows).toEqual([])
  })

  it('秒级采集状态只保留真实字符串列表项', () => {
    expect(normalizeTickCollectorStatus({
      active_symbols: ['BTC-USDT-SWAP', 123, true, ' ETH-USDT-SWAP '],
      errors: ['e1', 2, false, ' e2 '],
      book_channel: 123,
    })).toMatchObject({
      active_symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
      errors: ['e1', 'e2'],
      book_channel: 'books5',
    })
  })

  it('库存归一化保留缺失指标、周期范围和秒级特征柱计数标签', () => {
    const inventory = normalizeInventoryPayload({
      summary: {
        table_totals: {
          feature_bars_1s: 12,
        },
      },
      rows: [{
        symbol: 'btc-usdt',
        markets: {
          SWAP: {
            inst_id: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            gap_count: 3,
            oldest_time: '2026-05-01T00:00:00.000Z',
            newest_time: '2026-05-02T00:00:00.000Z',
            timeframes: [{
              timeframe: '1h',
              candle_count: 22,
              expected_candle_count: 25,
              gap_count: 3,
              coverage_ratio: 0.88,
              history_complete: false,
              oldest_timestamp: 1777593600000,
              newest_timestamp: 1777680000000,
              oldest_time: '2026-05-01T00:00:00.000Z',
              newest_time: '2026-05-02T00:00:00.000Z',
            }],
          },
        },
      }],
    })
    const market = inventory.rows[0].markets.SWAP

    expect(inventory.summary.table_totals.feature_bars_1s).toBe(12)
    expect(storageCountLabel('feature_bars_1s')).toBe('秒级特征柱')
    expect(market?.gap_count).toBe(3)
    expect(market ? inventoryMarketSummary(market) : '').toContain('缺失 3')
    expect(market?.timeframes[0]).toMatchObject({
      timeframe: '1H',
      gap_count: 3,
      oldest_timestamp: 1777593600000,
      newest_timestamp: 1777680000000,
    })
    expect(market ? inventoryTimeframeRangeLabel(market.timeframes[0]) : '').toContain('05/01')
    expect(market ? inventoryTimeframeGapLabel(market.timeframes[0]) : '').toBe('缺失 3')
    expect(market ? inventoryTimeframeCoverageLabel(market.timeframes[0]) : '').toBe('覆盖 88.0%')
  })

  it('库存 payload 只有时间戳时仍能显示库存和关注规则范围', () => {
    const inventory = normalizeInventoryPayload({
      rows: [{
        symbol: 'btc-usdt',
        markets: {
          SWAP: {
            inst_id: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            oldest_timestamp: 1777593600000,
            newest_timestamp: 1777680000000,
            timeframes: [{
              timeframe: '1h',
              candle_count: 22,
              expected_candle_count: 25,
              gap_count: 3,
              coverage_ratio: 0.88,
              history_complete: false,
              oldest_timestamp: 1777593600000,
              newest_timestamp: 1777680000000,
            }],
          },
        },
      }],
    })
    const market = inventory.rows[0].markets.SWAP
    const records = inventoryRowsToSyncRecords(inventory.rows)
    const row = {
      ...watchedSymbol({
        symbol: 'BTC-USDT',
        sync_plans: [syncPlan('1H', 90)],
      }),
      jobs: [],
      jobSummary: summarizeSyncProgress([]),
    } as WatchedRow
    const plan = buildPlanRows(row, 'SWAP', records, [])
      .find(item => item.timeframe === '1H')

    expect(market ? inventoryMarketSummary(market) : '').toContain('05/01')
    expect(market ? inventoryTimeframeRangeLabel(market.timeframes[0]) : '').toContain('05/01')
    expect(plan?.label).toContain('05/01')
    expect(plan?.start_ts).toBe(1777593600000)
    expect(plan?.end_ts).toBe(1777680000000)
  })

  it('Guardian 状态只读取直接数组和真实字符串字段', () => {
    expect(normalizeGuardianStatus({
      policy_summary: 123,
      current_inst_id: 456,
      rolling_window_timeframes: ['1m', 15, ' 15m '],
      backfill_queue_preview: {
        rows: [{
          task_id: 'sync-old',
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '1m',
          status: 'running',
        }],
      },
      last_errors: {
        rows: ['e1'],
      },
      last_sync_results: {
        rows: [{
          task_id: 'sync-result',
        }],
      },
      last_successful_run_at: { value: 1 },
    })).toMatchObject({
      policy_summary: '',
      current_inst_id: '',
      rolling_window_timeframes: ['1m', '15m'],
      backfill_queue_preview: [],
      last_errors: [],
      last_sync_results: [],
      last_successful_run_at: null,
    })
  })

  it('已有更新同步记录时忽略过期失败任务', () => {
    const [row] = buildWatchedRows(
      [watchedSymbol()],
      [syncJob({
        task_id: 'sync_old_failed',
        timeframe: '1H',
        target_timeframes: [],
        status: 'failed',
        progress: 0,
        error: 'runtime error: OKX API error 51000: Parameter bar error',
        updated_at: '2026-05-22T04:27:26.947475974+00:00',
        finished_at: '2026-05-22T04:27:26.947475974+00:00',
      })],
      [syncRecord({
        timeframe: '1H',
        candle_count: 8760,
        last_sync_time: '2026-05-23 10:11:23',
      })],
      [syncPlan('1H')],
    )

    expect(row.jobs).toHaveLength(0)
    expect(row.jobSummary.total).toBe(0)
    expect(row.jobSummary.failed).toBe(0)
  })

  it('保留当前活跃同步任务的进度', () => {
    const [row] = buildWatchedRows(
      [watchedSymbol()],
      [syncJob({
        task_id: 'sync_running',
        timeframe: '1m',
        source_timeframe: '1m',
        target_timeframes: ['1m', '1H'],
        status: 'running',
        progress: 42,
        fetched_count: 420,
        target_fetch_count: 1000,
        saved_count: 300,
        target_save_count: 1000,
        target_derive_count: 120,
        updated_at: '2026-05-23T08:00:00.000000000+00:00',
      })],
      [],
      [syncPlan('1H')],
    )

    expect(row.jobs).toHaveLength(1)
    expect(row.jobSummary.active).toBe(1)
    expect(row.jobSummary.statusLabel).toBe('同步中')
    expect(row.jobSummary.progress).toBe(42)
  })

  it('高频任务进度更新时复用未受影响的关注行引用', () => {
    const symbols = [
      watchedSymbol(),
      watchedSymbol({
        symbol: 'ETH-USDT',
        base_ccy: 'ETH',
        spot_inst_id: 'ETH-USDT',
        swap_inst_id: 'ETH-USDT-SWAP',
      }),
    ]
    const records = [
      syncRecord(),
      syncRecord({ inst_id: 'ETH-USDT-SWAP' }),
    ]
    const sources = buildWatchedRowSources(symbols, records, [syncPlan('1H')])
    const buildRows = createWatchedRowsBuilder()
    const first = buildRows(sources, [
      syncJob({
        task_id: 'sync_btc',
        inst_id: 'BTC-USDT-SWAP',
        progress: 10,
      }),
    ])
    const second = buildRows(sources, [
      syncJob({
        task_id: 'sync_btc',
        inst_id: 'BTC-USDT-SWAP',
        progress: 30,
      }),
    ])

    expect(second[0]).not.toBe(first[0])
    expect(second[0].jobSummary.progress).toBe(30)
    expect(second[1]).toBe(first[1])
  })

  it('同步记录局部替换时复用未受影响的行源引用', () => {
    const symbols = [
      watchedSymbol(),
      watchedSymbol({
        symbol: 'ETH-USDT',
        base_ccy: 'ETH',
        spot_inst_id: 'ETH-USDT',
        swap_inst_id: 'ETH-USDT-SWAP',
      }),
    ]
    const inventoryRows = [
      inventoryRow({
        symbol: 'DOGE-USDT',
        base_ccy: 'DOGE',
        inst_id: 'DOGE-USDT-SWAP',
        timeframes: ['1H'],
        watched: false,
        managed: false,
      }),
    ]
    const sourceBuilder = createWatchedRowSourcesBuilder()
    const plans = [syncPlan('1H')]
    let recordsByScope = buildSyncRecordScopeIndex([
      syncRecord(),
      syncRecord({ inst_id: 'ETH-USDT-SWAP' }),
      syncRecord({ inst_id: 'DOGE-USDT-SWAP' }),
    ])
    const first = sourceBuilder(symbols, [], plans, inventoryRows, recordsByScope)

    recordsByScope = replaceSyncRecordScopes(
      recordsByScope,
      [syncRecord({ inst_id: 'BTC-USDT-SWAP', candle_count: 999 })],
      new Set([syncRecordScopeKey('BTC-USDT-SWAP', 'SWAP')]),
    )
    const second = sourceBuilder(symbols, [], plans, inventoryRows, recordsByScope)

    expect(second[0]).not.toBe(first[0])
    expect(second[1]).toBe(first[1])
    expect(second[2]).toBe(first[2])
  })

  it('关注页同步进度刷新原地替换 scope 索引时保持局部替换语义', () => {
    const records = [
      syncRecord(),
      syncRecord({ inst_id: 'ETH-USDT-SWAP' }),
      syncRecord({ inst_id: 'DOGE-USDT-SWAP' }),
    ]
    const incoming = [syncRecord({ candle_count: 999, timeframe: '5m' })]
    const scopeKeys = new Set([syncRecordScopeKey('BTC-USDT-SWAP', 'SWAP')])

    const copiedIndex = replaceSyncRecordScopes(
      buildSyncRecordScopeIndex(records),
      incoming,
      scopeKeys,
    )
    const inPlaceIndex = buildSyncRecordScopeIndex(records)
    const returnedIndex = replaceSyncRecordScopesInPlace(inPlaceIndex, incoming, scopeKeys)

    expect(returnedIndex).toBe(inPlaceIndex)
    expect(inPlaceIndex).toEqual(copiedIndex)
    expect(inPlaceIndex.get('SWAP:BTC-USDT-SWAP')).toEqual(incoming)
    expect(inPlaceIndex.get('SWAP:ETH-USDT-SWAP')).toEqual([records[1]])
  })

  it('关注行预计算计划行与即时 scope 计算保持一致', () => {
    const enabledPlans = [syncPlan('1H'), syncPlan('5m')]
    const recordsByScope = buildSyncRecordScopeIndex([
      syncRecord({ timeframe: '1H', candle_count: 8760 }),
      syncRecord({ timeframe: '5m', candle_count: 288 }),
    ])
    const sourceBuilder = createWatchedRowSourcesBuilder()
    const rowBuilder = createWatchedRowsBuilder()
    const rows = rowBuilder(sourceBuilder([watchedSymbol()], [], enabledPlans, [], recordsByScope), [])
    const [row] = rows

    expect(row.planRowsByInstType?.SWAP).toEqual(
      buildPlanRowsFromScopeRecords(
        row,
        'SWAP',
        syncRecordsForScope(recordsByScope, row.swap_inst_id, 'SWAP'),
        enabledPlans,
      ),
    )
  })

  it('同步任务轮询可只派生可见关注行且保持完整列表切片语义', () => {
    const enabledPlans = [syncPlan('1H')]
    const watched = [
      watchedSymbol(),
      watchedSymbol({
        symbol: 'ETH-USDT',
        base_ccy: 'ETH',
        spot_inst_id: 'ETH-USDT',
        swap_inst_id: 'ETH-USDT-SWAP',
      }),
      watchedSymbol({
        symbol: 'DOGE-USDT',
        base_ccy: 'DOGE',
        spot_inst_id: 'DOGE-USDT',
        swap_inst_id: 'DOGE-USDT-SWAP',
      }),
    ]
    const recordsByScope = buildSyncRecordScopeIndex([
      syncRecord(),
      syncRecord({ inst_id: 'ETH-USDT-SWAP' }),
      syncRecord({ inst_id: 'DOGE-USDT-SWAP' }),
    ])
    const sources = createWatchedRowSourcesBuilder()(
      watched,
      [],
      enabledPlans,
      [],
      recordsByScope,
    )
    const visibleSources = sources.slice(1, 3)
    const jobs = [
      syncJob({ task_id: 'sync_eth', inst_id: 'ETH-USDT-SWAP', progress: 31 }),
      syncJob({ task_id: 'sync_doge', inst_id: 'DOGE-USDT-SWAP', progress: 62 }),
    ]

    const fullSample = buildWatchedRowsFromSources(sources, jobs)
    const visibleSample = buildWatchedRowsFromSources(visibleSources, jobs)
    expect(visibleSample.map(row => [row.symbol, row.jobSummary.progress])).toEqual(
      fullSample
        .slice(1, 3)
        .map(row => [row.symbol, row.jobSummary.progress]),
    )
  })

  it('派生模式旧规则能关联1m底座任务并显示计划行进度', () => {
    const [row] = buildWatchedRows(
      [watchedSymbol({
        sync_days: 90,
        sync_plans: [syncPlan('1H', 90)],
      })],
      [syncJob({
        task_id: 'sync_running',
        timeframe: '1m',
        source_timeframe: '1m',
        target_timeframes: ['1m', '1H'],
        status: 'running',
        progress: 42,
        fetched_count: 420,
        target_fetch_count: 1000,
        saved_count: 300,
        target_save_count: 1000,
        target_derive_count: 120,
        updated_at: '2026-05-23T08:00:00.000000000+00:00',
      })],
      [],
      [],
    )
    const planRows = buildPlanRows(row, 'SWAP', [], [])

    expect(row.jobs).toHaveLength(1)
    expect(planRows.map(plan => [plan.timeframe, plan.status, plan.label])).toEqual([
      ['1m', 'running', '42%'],
      ['1H', 'running', '42%'],
    ])
  })

  it('同一范围内活跃任务优先于更新的失败终态', () => {
    const [row] = buildWatchedRows(
      [watchedSymbol()],
      [
        syncJob({
          task_id: 'sync_running',
          timeframe: '1m',
          source_timeframe: '1m',
          target_timeframes: ['1m', '1H'],
          status: 'running',
          progress: 37,
          fetched_count: 370,
          target_fetch_count: 1000,
          updated_at: '2026-05-23T08:00:00.000000000+00:00',
        }),
        syncJob({
          task_id: 'sync_failed_newer',
          timeframe: '1m',
          source_timeframe: '1m',
          target_timeframes: ['1m', '1H'],
          status: 'failed',
          progress: 100,
          error: 'runtime error: OKX API error 51000: Parameter bar error',
          updated_at: '2026-05-23T08:01:00.000000000+00:00',
          finished_at: '2026-05-23T08:01:00.000000000+00:00',
        }),
      ],
      [],
      [syncPlan('1H')],
    )

    expect(row.jobs).toHaveLength(2)
    expect(row.jobSummary.active).toBe(1)
    expect(row.jobSummary.failed).toBe(0)
    expect(row.jobSummary.statusLabel).toBe('同步中')
    expect(row.jobSummary.progress).toBe(37)
  })

  it('同步任务列表保留活跃和观察中的终态任务，隐藏过期和已覆盖失败任务', () => {
    const now = Date.parse('2026-05-23T08:10:00.000Z')
    const observedDeadlines = new Map([['sync_observed_failed', now + 60_000]])
    const jobs = visibleSyncJobs([
      syncJob({
        task_id: 'sync_running',
        status: 'running',
        updated_at: '2026-05-23T06:00:00.000Z',
      }),
      syncJob({
        task_id: 'sync_superseded_failed',
        status: 'failed',
        error: 'old error',
        updated_at: '2026-05-23T07:00:00.000Z',
        finished_at: '2026-05-23T07:00:00.000Z',
      }),
      syncJob({
        task_id: 'sync_observed_failed',
        timeframe: '5m',
        target_timeframes: ['5m'],
        status: 'failed',
        error: 'recently submitted failed',
        updated_at: '2026-05-23T07:00:00.000Z',
        finished_at: '2026-05-23T07:00:00.000Z',
      }),
      syncJob({
        task_id: 'sync_stale_done',
        status: 'completed',
        updated_at: '2026-05-23T07:00:00.000Z',
        finished_at: '2026-05-23T07:00:00.000Z',
      }),
    ], [
      syncRecord({
        timeframe: '1H',
        candle_count: 12,
        last_sync_time: '2026-05-23T07:30:00.000Z',
      }),
    ], observedDeadlines, now)

    expect(jobs.map(job => job.task_id)).toEqual(['sync_observed_failed', 'sync_running'])
  })

  it('同步任务合并时同 task_id 只保留最新状态', () => {
    const now = Date.parse('2026-05-23T08:10:00.000Z')
    const merged = mergeSyncJobs([
      syncJob({
        task_id: 'sync_same',
        status: 'running',
        progress: 40,
        updated_at: '2026-05-23T08:05:00.000Z',
      }),
    ], [
      syncJob({
        task_id: 'sync_same',
        status: 'running',
        progress: 20,
        updated_at: '2026-05-23T08:00:00.000Z',
      }),
      syncJob({
        task_id: 'sync_new',
        status: 'queued',
        progress: 0,
        updated_at: '2026-05-23T08:06:00.000Z',
      }),
    ], [], new Map(), now)

    expect(merged.map(job => [job.task_id, job.progress])).toEqual([
      ['sync_new', 0],
      ['sync_same', 40],
    ])
  })

  it('关注规则默认表单使用 Guardian 全局周期并套用统一天数', () => {
    const form = defaultWatchRuleForm([
      {
        timeframe: '1H',
        enabled: true,
        bootstrap_days: 60,
        archive_mode: 'rolling',
      },
    ], 120)

    expect(form).toMatchObject({
      syncSpot: true,
      syncSwap: true,
      archiveAll: false,
      autoSync: true,
      syncDays: 120,
    })
    expect(form.syncPlans.find(plan => plan.timeframe === '1H')).toMatchObject({
      enabled: true,
      bootstrap_days: 120,
      archive_mode: 'rolling',
    })
    expect(form.syncPlans.every(plan => plan.bootstrap_days === 120)).toBe(true)
  })

  it('库内未接管标的编辑表单使用库存周期生成可提交规则', () => {
    const inventory = inventoryRow({
      symbol: 'ETH-USDT',
      inst_id: 'ETH-USDT-SWAP',
      watched: false,
      managed: false,
      timeframes: ['1m', '3m'],
    })
    const [row] = buildWatchedRows([], [], inventoryRowsToSyncRecords([inventory]), [], [inventory])
    const form = watchRuleFormFromRow(row, [], 30)

    expect(form).toMatchObject({
      syncSpot: false,
      syncSwap: true,
      archiveAll: false,
      autoSync: true,
      syncDays: 90,
    })
    expect(form.syncPlans.filter(plan => plan.enabled).map(plan => [plan.timeframe, plan.bootstrap_days])).toEqual([
      ['1m', 90],
      ['3m', 90],
    ])
  })

  it('关注规则保存和修复共用同步任务摘要', () => {
    const result = {
      started_count: 2,
      reused_count: 1,
      exact_gap_jobs: 3,
      rule_jobs: 4,
    }

    expect(watchRuleSavedAction(false, true)).toBe('已接管库内标的规则')
    expect(syncTaskSubmissionSummary(result)).toBe('新增 2 个任务，复用 1 个任务，精确缺口 3 个，规则同步 4 个')
    expect(repairWatchedSymbolMessage('BTC-USDT', result)).toBe(
      'BTC-USDT 已按关注规则提交补齐，新增 2 个任务，复用 1 个任务，精确缺口 3 个，规则同步 4 个',
    )
  })

  it('采集运行参数按字段值判断是否需要保存', () => {
    expect(sameSyncRuntimeSettings(
      { max_concurrent_symbols: 3, batch_size: 100 },
      { max_concurrent_symbols: 3, batch_size: 100 },
    )).toBe(true)
    expect(sameSyncRuntimeSettings(
      { max_concurrent_symbols: 3, batch_size: 100 },
      { max_concurrent_symbols: 4, batch_size: 100 },
    )).toBe(false)
  })

  it('库存表计数把 total 固定在首位并过滤零值', () => {
    expect(buildInventoryTableTotals({
      ...emptyInventorySummaryForTest(),
      table_totals: {
        sync_records: 9,
        total: 3,
        candles: 12,
        empty: 0,
      },
    })).toEqual([
      { key: 'total', value: 3 },
      { key: 'candles', value: 12 },
      { key: 'sync_records', value: 9 },
    ])
  })

  it('Guardian 展示文案只保留有效错误并组合当前目标', () => {
    const status = normalizeGuardianStatus({
      current_inst_id: 'BTC-USDT-SWAP',
      current_timeframe: '1H',
      current_mode: 'repair',
      last_errors: [' first ', { message: ' second ' }, { message: 12 }, 0],
    })

    expect(guardianErrorMessages(status)).toEqual(['first', 'second'])
    expect(guardianCurrentTargetText(status)).toBe('BTC-USDT-SWAP · 1H · repair')
    expect(guardianCurrentTargetText(null)).toBe('--')
  })

  it('关注规则按钮和启用数量状态由纯函数派生', () => {
    expect(activeSyncJobs([
      syncJob({ task_id: 'queued', status: 'queued' }),
      syncJob({ task_id: 'running', status: 'running' }),
      syncJob({ task_id: 'completed', status: 'completed' }),
    ]).map(job => job.task_id)).toEqual(['queued', 'running'])
    expect(canOpenWatchRuleDialog(' btc ')).toBe(true)
    expect(canOpenWatchRuleDialog('')).toBe(false)
    expect(canSubmitWatchRuleDialog({
      pendingSymbol: 'BTC-USDT',
      syncSpot: false,
      syncSwap: true,
      syncPlans: [syncPlan('1H')],
    })).toBe(true)
    expect(canSubmitWatchRuleDialog({
      pendingSymbol: 'BTC-USDT',
      syncSpot: false,
      syncSwap: false,
      syncPlans: [syncPlan('1H')],
    })).toBe(false)
    expect(watchRuleSubmitButtonLabel(true, true)).toBe('保存中')
    expect(watchRuleSubmitButtonLabel(false, true)).toBe('保存规则并同步')
    expect(watchRuleSubmitButtonLabel(false, false)).toBe('保存关注规则')
    expect(countEnabledInstruments([
      watchedSymbol({ sync_spot: true, sync_swap: true }),
      watchedSymbol({ sync_spot: false, sync_swap: true }),
    ])).toBe(3)
  })

  it('Guardian 计划和 tab 提示由工具函数生成', () => {
    const plans = enabledSyncPlansFromGuardian([
      { timeframe: '1H', enabled: true, bootstrap_days: 90, archive_mode: 'rolling' },
      { timeframe: '5m', enabled: false, bootstrap_days: 90, archive_mode: 'rolling' },
    ])

    expect(plans.map(plan => plan.timeframe)).toEqual(['1H'])
    expect(managedPlanLabelText(plans)).toBe('1H')
    expect(dataCenterTabDescription([
      { key: 'watchlist', label: '数据标的', description: '关注规则' },
      { key: 'inventory', label: '库存', description: '库存覆盖' },
    ], 'inventory')).toBe('库存覆盖')
  })

  it('库存缺口修复参数和方式标签使用统一规则', () => {
    const payload = {
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP' as const,
      timeframe: '1H' as const,
      start_ts: 1000,
      end_ts: 2000,
    }

    expect(inventoryGapKey(payload.inst_id, payload.inst_type, payload.timeframe)).toBe('BTC-USDT-SWAP:SWAP:1H')
    expect(hasValidInventoryGapRange(payload)).toBe(true)
    expect(hasValidInventoryGapRange({ ...payload, end_ts: 999 })).toBe(false)
    expect(hasValidInventoryGapRange({ ...payload, start_ts: Number.NaN })).toBe(false)
    expect(gapRepairMethodLabel({ paginated_ranges: 2, historical_zip_ranges: 1 })).toBe('分页 2 段 / 历史 zip 1 段')
  })
})

function emptyInventorySummaryForTest() {
  return {
    symbol_count: 0,
    managed_symbol_count: 0,
    managed_market_count: 0,
    watched_symbol_count: 0,
    watched_list_count: 0,
    watched_market_count: 0,
    orphan_symbol_count: 0,
    total_candles: 0,
    total_timeframe_records: 0,
    table_totals: {},
  }
}

function watchedSymbol(overrides: Partial<WatchedSymbol> = {}): WatchedSymbol {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    spot_inst_id: 'BTC-USDT',
    swap_inst_id: 'BTC-USDT-SWAP',
    sync_spot: false,
    sync_swap: true,
    archive_all_history: false,
    sync_days: 30,
    sync_plans: [syncPlan('1H')],
    created_at: '2026-05-22T00:00:00.000000000+00:00',
    updated_at: '2026-05-22T00:00:00.000000000+00:00',
    ...overrides,
  }
}

function syncPlan(
  timeframe: WatchedSymbolSyncPlan['timeframe'],
  bootstrapDays = 30,
  archiveMode: WatchedSymbolSyncPlan['archive_mode'] = 'rolling',
): WatchedSymbolSyncPlan {
  return {
    timeframe,
    enabled: true,
    bootstrap_days: bootstrapDays,
    archive_mode: archiveMode,
  }
}

function syncRecord(overrides: Partial<SyncRecord> = {}): SyncRecord {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    last_sync_time: '2026-05-23 10:11:23',
    oldest_time: '2025-05-23T11:00:00Z',
    newest_time: '2026-05-23T10:00:00Z',
    candle_count: 8760,
    history_complete: false,
    last_sync_mode: 'window',
    ...overrides,
  }
}

function inventoryRow(options: {
  timeframes: Array<WatchedSymbolSyncPlan['timeframe']>
  symbol?: string
  base_ccy?: string
  inst_id?: string
  watched?: boolean
  managed?: boolean
}): InventoryRow {
  const symbol = options.symbol ?? 'BTC-USDT'
  const baseCcy = options.base_ccy ?? symbol.split('-')[0] ?? symbol
  const instId = options.inst_id ?? `${symbol}-SWAP`
  const watched = options.watched ?? true
  const managed = options.managed ?? true
  return {
    symbol,
    base_ccy: baseCcy,
    managed,
    watched,
    orphan: false,
    candle_count: 20,
    timeframe_record_count: options.timeframes.length,
    storage_counts: { candles: 20, total: 20 },
    markets: {
      SWAP: {
        inst_id: instId,
        inst_type: 'SWAP',
        managed,
        watched,
        timeframe_count: options.timeframes.length,
        candle_count: 20,
        gap_count: 2,
        history_complete_count: 1,
        oldest_time: '2026-05-01T00:00:00.000Z',
        newest_time: '2026-05-02T00:00:00.000Z',
        last_sync_time: '2026-05-02T00:00:00.000Z',
        timeframes: options.timeframes.map(timeframe => ({
          timeframe,
          managed: true,
          candle_count: timeframe === '5m' ? 10 : 20,
          expected_candle_count: timeframe === '5m' ? 12 : 20,
          gap_count: timeframe === '5m' ? 2 : 0,
          coverage_ratio: timeframe === '5m' ? 10 / 12 : 1,
          history_complete: timeframe !== '5m',
          last_sync_mode: 'derive',
          last_sync_time: '2026-05-02T00:00:00.000Z',
          oldest_timestamp: 1777593600000,
          newest_timestamp: 1777680000000,
          oldest_time: '2026-05-01T00:00:00.000Z',
          newest_time: '2026-05-02T00:00:00.000Z',
        })),
      },
    },
  }
}

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
