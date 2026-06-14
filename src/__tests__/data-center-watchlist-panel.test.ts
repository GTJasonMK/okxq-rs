import { defineComponent, nextTick, type PropType } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import DataCenterWatchlistPanel from '@/components/data-center/DataCenterWatchlistPanel.vue'
import type { WatchedRow } from '@/types/dataCenter'
import type { WatchedRowSource } from '@/utils/dataCenter'

const ROW_HEIGHT = 230

const RowStub = defineComponent({
  name: 'DataCenterWatchlistRow',
  props: {
    row: {
      type: Object as PropType<WatchedRow>,
      required: true,
    },
  },
  template: '<article class="dc-row-stub">{{ row.symbol }}</article>',
})

const ToolbarStub = defineComponent({
  name: 'DataCenterWatchlistToolbar',
  template: '<div class="dc-toolbar-stub"></div>',
})

describe('DataCenterWatchlistPanel 大列表渲染', () => {
  it('关注列表只挂载视窗附近行，避免同步后期全量 DOM 重渲染', () => {
    const rows = perfWatchedRows(3_000)

    const wrapper = mount(DataCenterWatchlistPanel, {
      props: panelProps(rows),
      global: {
        stubs: {
          DataCenterWatchlistRow: RowStub,
          DataCenterWatchlistToolbar: ToolbarStub,
        },
      },
    })

    expect(wrapper.findAll('.dc-row-stub').length).toBeLessThanOrEqual(24)

    wrapper.unmount()
  })

  it('关注列表滚动时切换可见窗口并保持挂载行数有界', async () => {
    const rows = perfWatchedRows(300)
    const wrapper = mount(DataCenterWatchlistPanel, {
      props: panelProps(rows),
      global: {
        stubs: {
          DataCenterWatchlistRow: RowStub,
          DataCenterWatchlistToolbar: ToolbarStub,
        },
      },
    })
    const viewport = wrapper.find('.dc-content')
    Object.defineProperty(viewport.element, 'clientHeight', {
      value: ROW_HEIGHT * 2,
      configurable: true,
    })
    ;(viewport.element as HTMLElement).scrollTop = ROW_HEIGHT * 120
    await viewport.trigger('scroll')
    await nextTick()

    expect(wrapper.text()).toContain('PERF-0120-USDT')
    expect(wrapper.findAll('.dc-row-stub').length).toBeLessThanOrEqual(24)

    wrapper.unmount()
  })
})

function panelProps(watchedRows: WatchedRow[]) {
  return {
    active: true,
    newSymbol: '',
    adding: false,
    canOpenRuleDialog: false,
    loading: false,
    guardianRunning: false,
    visibleSymbolsCount: watchedRows.length,
    watchedSymbolsCount: watchedRows.length,
    enabledInstrumentCount: watchedRows.length,
    activeJobsCount: 0,
    managedPlanLabels: '',
    message: '',
    error: '',
    watchedRowSources: watchedRowSourcesFromRows(watchedRows),
    syncJobs: [],
    enabledPlans: [],
    repairingSymbol: '',
    gapRepairingKey: '',
    deletingSymbol: '',
  }
}

function perfWatchedRows(count: number): WatchedRow[] {
  return Array.from({ length: count }, (_, index) => {
    const base = `PERF-${index.toString().padStart(4, '0')}`
    return {
      symbol: `${base}-USDT`,
      base_ccy: base,
      spot_inst_id: `${base}-USDT`,
      swap_inst_id: `${base}-USDT-SWAP`,
      sync_spot: true,
      sync_swap: true,
      archive_all_history: false,
      sync_days: 30,
      sync_plans: [],
      created_at: '2026-05-22T00:00:00.000Z',
      updated_at: '2026-05-22T00:00:00.000Z',
      jobs: [],
      jobSummary: {
        total: 0,
        queued: 0,
        running: 0,
        completed: 0,
        failed: 0,
        cancelled: 0,
        active: 0,
        progress: 0,
        statusLabel: '空闲',
        phaseLabel: '',
        primaryText: '',
        secondaryText: '',
        taskText: '',
        segments: [],
        fetched: 0,
        targetFetch: 0,
        saved: 0,
        targetSave: 0,
        derived: 0,
        targetDerive: 0,
        batches: 0,
        targetBatches: 0,
        apiCalls: 0,
      },
      planRowsByInstType: {
        SPOT: [],
        SWAP: [],
      },
    }
  })
}

function watchedRowSourcesFromRows(rows: WatchedRow[]): WatchedRowSource[] {
  return rows.map((row) => {
    const { jobs: _jobs, jobSummary: _jobSummary, ...sourceRow } = row
    return {
      row: sourceRow,
      scopes: [],
      effectiveTimeframes: new Set(),
      enabledPlans: [],
    }
  })
}
