import { nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import DataCenterInventoryPanel from '@/components/data-center/DataCenterInventoryPanel.vue'
import type {
  InventoryMarket,
  InventoryRow,
  InventorySummary,
  InventoryTimeframeRecord,
} from '@/types/dataCenter'

const ROW_HEIGHT = 360

describe('DataCenterInventoryPanel 大列表渲染', () => {
  it('库存列表只挂载视窗附近行，避免库存页全量 DOM 渲染', () => {
    const rows = perfInventoryRows(1_500)

    const wrapper = mount(DataCenterInventoryPanel, {
      props: panelProps(rows),
    })

    expect(wrapper.findAll('.dc-inventory-row').length).toBeLessThanOrEqual(18)

    wrapper.unmount()
  })

  it('库存列表滚动时切换可见窗口并保持挂载行数有界', async () => {
    const rows = perfInventoryRows(300)
    const wrapper = mount(DataCenterInventoryPanel, {
      props: panelProps(rows),
    })
    const viewport = wrapper.find('.dc-inventory-table')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = ROW_HEIGHT * 120
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('PERF-0120-USDT')
    expect(wrapper.findAll('.dc-inventory-row').length).toBeLessThanOrEqual(18)

    wrapper.unmount()
  })
})

function panelProps(rows: InventoryRow[]) {
  return {
    active: true,
    rows,
    summary: inventorySummary(rows.length),
    tableTotals: [],
    loading: false,
    rebuilding: false,
    rebuildProgress: null,
    activeJobsCount: 0,
    message: '',
    error: '',
    gapRepairingKey: '',
  }
}

function inventorySummary(count: number): InventorySummary {
  return {
    symbol_count: count,
    managed_symbol_count: count,
    managed_market_count: count,
    watched_symbol_count: count,
    watched_list_count: count,
    watched_market_count: count,
    orphan_symbol_count: 0,
    total_candles: count * 4_000,
    total_timeframe_records: count * 4,
    table_totals: {},
  }
}

function perfInventoryRows(count: number): InventoryRow[] {
  return Array.from({ length: count }, (_, index) => {
    const base = `PERF-${index.toString().padStart(4, '0')}`
    const symbol = `${base}-USDT`
    const market = inventoryMarket(`${symbol}-SWAP`)
    return {
      symbol,
      base_ccy: base,
      managed: true,
      watched: true,
      orphan: false,
      candle_count: market.candle_count,
      timeframe_record_count: market.timeframe_count,
      storage_counts: {
        total: market.candle_count,
        candles: market.candle_count,
      },
      markets: {
        SWAP: market,
      },
    }
  })
}

function inventoryMarket(instId: string): InventoryMarket {
  const timeframes = ['1m', '5m', '15m', '1H'] as const
  const records = timeframes.map((timeframe, index) => inventoryTimeframe(timeframe, index))
  return {
    inst_id: instId,
    inst_type: 'SWAP',
    managed: true,
    watched: true,
    timeframe_count: records.length,
    candle_count: records.reduce((total, item) => total + item.candle_count, 0),
    gap_count: records.reduce((total, item) => total + item.gap_count, 0),
    history_complete_count: records.filter(item => item.history_complete).length,
    oldest_time: '2026-05-01T00:00:00.000Z',
    newest_time: '2026-05-02T00:00:00.000Z',
    last_sync_time: '2026-05-02T00:00:00.000Z',
    timeframes: records,
  }
}

function inventoryTimeframe(
  timeframe: InventoryTimeframeRecord['timeframe'],
  index: number,
): InventoryTimeframeRecord {
  return {
    timeframe,
    managed: true,
    candle_count: 1_000 + index,
    expected_candle_count: 1_005 + index,
    gap_count: index % 2,
    coverage_ratio: 0.99,
    history_complete: index % 2 === 0,
    last_sync_mode: 'window',
    last_sync_time: '2026-05-02T00:00:00.000Z',
    oldest_timestamp: 1777593600000,
    newest_timestamp: 1777680000000,
    oldest_time: '2026-05-01T00:00:00.000Z',
    newest_time: '2026-05-02T00:00:00.000Z',
  }
}
