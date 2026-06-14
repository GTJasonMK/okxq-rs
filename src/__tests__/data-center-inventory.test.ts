import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterInventory } from '@/composables/useDataCenterInventory'
import type { SyncRecord } from '@/types'
import type { InventoryCacheRebuildProgress, InventoryRow, InventorySummary } from '@/types/dataCenter'

vi.mock('@/api/market', () => ({
  fetchInventory: vi.fn(),
  startInventoryCacheRebuild: vi.fn(),
  fetchInventoryCacheRebuildStatus: vi.fn(),
}))

const fetchInventoryMock = vi.mocked(marketApi.fetchInventory)
const startInventoryCacheRebuildMock = vi.mocked(marketApi.startInventoryCacheRebuild)
const fetchInventoryCacheRebuildStatusMock = vi.mocked(marketApi.fetchInventoryCacheRebuildStatus)

describe('useDataCenterInventory', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('应用库存 payload 时同步更新库存行、summary 和同步记录', () => {
    const inventory = useDataCenterInventory(feedback())
    const payload = inventoryPayload({
      summary: inventorySummary({
        table_totals: {
          total: 3,
          candles: 5,
        },
      }),
    })

    const records = inventory.applyInventoryPayload(payload)

    expect(inventory.inventorySummary.value.symbol_count).toBe(1)
    expect(inventory.inventoryRows.value).toEqual(payload.rows)
    expect(inventory.syncRecords.value).toEqual(records)
    expect(inventory.syncRecordsByScope.value.get('SWAP:BTC-USDT-SWAP')).toEqual(records)
    expect(records).toMatchObject([
      {
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '1H',
        candle_count: 5,
      },
    ])
    expect(inventory.inventoryTableTotals.value).toEqual([
      { key: 'total', value: 3 },
      { key: 'candles', value: 5 },
    ])
  })

  it('局部替换同步记录 scope 时保留未命中的库存索引', () => {
    const inventory = useDataCenterInventory(feedback())
    inventory.applyInventoryPayload(inventoryPayload({
      rows: [
        inventoryRow(),
        inventoryRow({ symbol: 'DOGE-USDT', inst_id: 'DOGE-USDT-SWAP' }),
      ],
    }))
    const refreshed = syncRecord({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '5m',
      candle_count: 30,
    })

    const records = inventory.replaceSyncRecordScopes(
      [refreshed],
      new Set(['SWAP:BTC-USDT-SWAP']),
    )

    expect(records).toEqual([refreshed])
    expect(inventory.syncRecords.value).toEqual([refreshed])
    expect(inventory.syncRecordsByScope.value.get('SWAP:BTC-USDT-SWAP')).toEqual([refreshed])
    expect(inventory.syncRecordsByScope.value.get('SWAP:DOGE-USDT-SWAP')).toMatchObject([
      { inst_id: 'DOGE-USDT-SWAP', timeframe: '1H' },
    ])
  })

  it('全库扫描完成后刷新库存并写入扫描摘要', async () => {
    const state = feedback()
    const inventory = useDataCenterInventory(state)
    const running = rebuildProgress({ status: 'running', progress: 10 })
    const completed = rebuildProgress({
      status: 'completed',
      progress: 100,
      sync_records_rebuilt: 2,
      stale_sync_records_deleted: 1,
      cached_candles_total: 1234,
    })
    fetchInventoryMock.mockResolvedValue(inventoryPayload({
      summary: inventorySummary({ symbol_count: 2 }),
      rows: [inventoryRow({ symbol: 'ETH-USDT', inst_id: 'ETH-USDT-SWAP' })],
    }))
    startInventoryCacheRebuildMock.mockResolvedValue({
      reused_existing: false,
      progress: running,
    })
    fetchInventoryCacheRebuildStatusMock.mockResolvedValue({ progress: completed })

    await inventory.rebuildInventoryCache()

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(startInventoryCacheRebuildMock).toHaveBeenCalledWith({ concurrency: 8 })
    expect(fetchInventoryCacheRebuildStatusMock).toHaveBeenCalledTimes(1)
    expect(fetchInventoryMock).toHaveBeenCalledTimes(1)
    expect(inventory.inventoryRebuildProgress.value).toMatchObject(completed)
    expect(inventory.inventoryRows.value[0].symbol).toBe('ETH-USDT')
    expect(state.message.value).toBe('全库扫描完成：重建 2 条周期缓存，清理陈旧缓存 1 条，缓存 K 线 1,234 根')
    expect(state.error.value).toBe('')
    expect(inventory.inventoryRebuilding.value).toBe(false)
  })

})

function feedback() {
  return {
    message: ref(''),
    error: ref(''),
    clearFeedback: vi.fn(),
  }
}

function inventoryPayload(overrides: Partial<{ summary: InventorySummary; rows: InventoryRow[] }> = {}) {
  return {
    summary: inventorySummary(),
    rows: [inventoryRow()],
    ...overrides,
  }
}

function inventorySummary(overrides: Partial<InventorySummary> = {}): InventorySummary {
  return {
    symbol_count: 1,
    managed_symbol_count: 1,
    managed_market_count: 1,
    watched_symbol_count: 1,
    watched_list_count: 1,
    watched_market_count: 1,
    orphan_symbol_count: 0,
    total_candles: 5,
    total_timeframe_records: 1,
    table_totals: {},
    ...overrides,
  }
}

function inventoryRow(overrides: Partial<{ symbol: string; inst_id: string }> = {}): InventoryRow {
  const symbol = overrides.symbol ?? 'BTC-USDT'
  const instId = overrides.inst_id ?? 'BTC-USDT-SWAP'
  return {
    symbol,
    base_ccy: symbol.split('-')[0],
    managed: true,
    watched: true,
    orphan: false,
    candle_count: 5,
    timeframe_record_count: 1,
    storage_counts: { total: 5 },
    markets: {
      SWAP: {
        inst_id: instId,
        inst_type: 'SWAP',
        managed: true,
        watched: true,
        timeframe_count: 1,
        candle_count: 5,
        gap_count: 0,
        history_complete_count: 1,
        oldest_time: '2026-01-01T00:00:00.000Z',
        newest_time: '2026-01-01T04:00:00.000Z',
        last_sync_time: '2026-01-01T04:00:00.000Z',
        timeframes: [{
          timeframe: '1H',
          managed: true,
          candle_count: 5,
          expected_candle_count: 5,
          gap_count: 0,
          coverage_ratio: 1,
          history_complete: true,
          last_sync_mode: 'full',
          last_sync_time: '2026-01-01T04:00:00.000Z',
          oldest_timestamp: 1767225600000,
          newest_timestamp: 1767240000000,
          oldest_time: '2026-01-01T00:00:00.000Z',
          newest_time: '2026-01-01T04:00:00.000Z',
        }],
      },
    },
  }
}

function syncRecord(overrides: Partial<SyncRecord> = {}): SyncRecord {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    last_sync_time: '2026-01-01T04:00:00.000Z',
    oldest_timestamp: 1767225600000,
    newest_timestamp: 1767240000000,
    oldest_time: '2026-01-01T00:00:00.000Z',
    newest_time: '2026-01-01T04:00:00.000Z',
    candle_count: 5,
    expected_candle_count: 5,
    gap_count: 0,
    coverage_ratio: 1,
    history_complete: true,
    last_sync_mode: 'full',
    ...overrides,
  }
}

function rebuildProgress(overrides: Partial<InventoryCacheRebuildProgress> = {}): InventoryCacheRebuildProgress {
  return {
    task_id: 'inventory-rebuild',
    status: 'running',
    phase: 'scan',
    progress: 0,
    message: '',
    started_at: '2026-01-01T00:00:00.000Z',
    updated_at: '2026-01-01T00:00:00.000Z',
    finished_at: null,
    error: '',
    processed_candles: 0,
    target_candles: 0,
    processed_groups: 0,
    target_groups: 0,
    scan_concurrency: 8,
    candle_groups_scanned: 0,
    sync_records_rebuilt: 0,
    stale_sync_records_deleted: 0,
    sync_records_total: 0,
    cached_candles_total: 0,
    ...overrides,
  }
}
