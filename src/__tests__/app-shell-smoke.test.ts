import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import App from '@/App.vue'
import router from '@/router'
import { sidebarNavGroups } from '@/config/navigation'

const invokeMock = vi.mocked(invoke)

const ROUTE_SMOKE_CASES = [
  { path: '/', label: '仪表盘', marker: '仪表盘' },
  { path: '/market', label: '行情', marker: 'BTC-USDT' },
  { path: '/data-center', label: '数据中心', marker: '数据中心' },
  { path: '/trading', label: '交易', marker: '交易中心' },
  { path: '/live-strategy', label: '实盘策略', marker: '策略运行' },
  { path: '/backtest', label: '回测', marker: '回测' },
  { path: '/risk', label: '风险', marker: '风险监控' },
  { path: '/scanner', label: '扫描', marker: '扫描' },
  { path: '/research', label: '研究', marker: '研究平台' },
  { path: '/trend-research', label: '趋势研究', marker: '趋势研究' },
  { path: '/journal', label: '日志', marker: '交易日志' },
  { path: '/assistant', label: 'AI', marker: 'AI 助手' },
  { path: '/settings', label: '设置', marker: '设置' },
]

describe('App Shell 路由挂载 smoke', () => {
  beforeEach(async () => {
    window.localStorage.clear()
    setActivePinia(createPinia())
    invokeMock.mockImplementation(mockInvoke)
    await router.push('/')
    await router.isReady()
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('导航配置中的所有主路由都能挂载对应页面', async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [createPinia(), router],
        stubs: {
          KlineChart: chartStub('KlineChart'),
          EquityCandleChart: chartStub('EquityCandleChart'),
          DrawdownChart: chartStub('DrawdownChart'),
          BenchmarkChart: chartStub('BenchmarkChart'),
        },
      },
    })
    await settle()

    const configuredPaths = sidebarNavGroups.flatMap(group => group.items.map(item => item.path))
    expect(configuredPaths).toEqual(ROUTE_SMOKE_CASES.map(item => item.path))

    for (const item of ROUTE_SMOKE_CASES) {
      const link = wrapper.findAll('.nav-item').find(candidate => candidate.text().includes(item.label))
      expect(link, `missing sidebar link for ${item.path}`).toBeTruthy()

      await router.push(item.path)
      await router.isReady()
      await settle()

      expect(router.currentRoute.value.path).toBe(item.path)
      expect(wrapper.text()).toContain(item.marker)
      expect(wrapper.find('.app-content').exists()).toBe(true)
    }

    wrapper.unmount()
  })
})

function chartStub(name: string) {
  return defineComponent({
    name,
    setup(_, { slots }) {
      return () => h('div', { class: `stub-${name}` }, slots.default?.())
    },
  })
}

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await flushPromises()
  }
}

function mockInvoke(command: string, args?: unknown) {
  const params = isRecord(args) ? args : {}
  if (command === 'local_api_request') {
    const req = params.req as { path?: string; method?: string; params?: Record<string, unknown>; body?: unknown } | undefined
    return Promise.resolve({ code: 0, data: localApiData(req?.path ?? '', req?.method ?? 'GET', req) })
  }
  if (command === 'get_preference') return Promise.resolve(null)
  if (command === 'update_preferences') return Promise.resolve({})
  if (command === 'get_okx_config') {
    return Promise.resolve({
      demo: {},
      live: {},
      use_simulated: true,
      is_configured: false,
      proxy_url: '',
      effective_proxy_url: '',
    })
  }
  if (command === 'get_assistant_config') {
    return Promise.resolve({
      enabled: true,
      configured: false,
      base_url: '',
      api_key: '',
      model: '',
      provider_name: '',
    })
  }
  return Promise.resolve({})
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function localApiData(path: string, method: string, req?: { params?: Record<string, unknown>; body?: unknown }) {
  if (path === '/health') return { ok: true, status: 'ok' }
  if (path === '/status') return { okx: { mode: 'simulated' }, database: { connected: true } }

  if (path === '/api/market/watched-symbols') return [watchedSymbol()]
  if (path === '/api/market/inventory') return { summary: inventorySummary(), rows: [inventoryRow()] }
  if (path === '/api/market/sync/jobs') return { jobs: [] }
  if (path === '/api/market/sync/config') return syncRuntimeConfig()
  if (path === '/api/market/data-guardian/status') return guardianStatus()
  if (path === '/api/market/data-guardian/config') return guardianConfig()
  if (path === '/api/market/data-guardian/run-now') return guardianStatus()
  if (path === '/api/market/tick-collector/status') return tickCollectorStatus()
  if (path === '/api/market/tick-collector/start') return { message: 'started', status: tickCollectorStatus({ running: true }) }
  if (path === '/api/market/tick-collector/stop') return { message: 'stopped', status: tickCollectorStatus() }
  if (path.startsWith('/api/market/candles/')) return { candles: [candle()] }
  if (path.startsWith('/api/market/ticker/')) return ticker()
  if (path.startsWith('/api/market/orderbook/')) return { orderbook: orderbook() }
  if (path.startsWith('/api/market/trades/')) return { trades: [] }
  if (path === '/api/market/symbols') return { symbols: [] }
  if (path === '/api/market/tickers') return { tickers: [] }
  if (path === '/api/market/alerts') return { alerts: [] }

  if (path === '/api/trading/account') return { total_eq: 1000, iso_eq: 0, adj_eq: 0, usdt_balance: 1000 }
  if (path === '/api/trading/positions') return []
  if (path === '/api/trading/orders') return []
  if (path === '/api/trading/fills') return []
  if (path === '/api/trading/cost-basis') return []
  if (path === '/api/trading/local-fills') return []
  if (path === '/api/trading/performance') return []
  if (path === '/api/trading/risk-control') return {}
  if (path === '/api/trading/risk-summary') return {}

  if (path === '/api/live/available-strategies') return [{ id: 'multi_timeframe_dual_v12', name: 'V20' }]
  if (path === '/api/live/status') return liveStatus()
  if (path === '/api/live/execution-plans') return []
  if (path === '/api/live/orders') return []
  if (path === '/api/live/equity') return { run_id: 'run', count: 0, snapshots: [], daily: [] }
  if (path === '/api/live/execution-logs') return []
  if (path === '/api/live/start' || path === '/api/live/stop') return liveStatus()

  if (path === '/api/backtest/strategies') return [{ id: 'multi_timeframe_dual_v12', name: 'V20' }]
  if (path === '/api/backtest/history') return []
  if (path.startsWith('/api/backtest/')) return {}

  if (path === '/api/risk/snapshots') return []
  if (path === '/api/risk/metrics') {
    return {
      has_data: false,
      message: '',
      data_points: 0,
      var_95: 0,
      var_99: 0,
      parametric_var_95: 0,
      sharpe_ratio: 0,
      sortino_ratio: 0,
      max_drawdown: 0,
      max_drawdown_duration: 0,
      current_drawdown: 0,
      peak_equity: 0,
      latest_equity: 0,
    }
  }
  if (path === '/api/risk/drawdown') {
    return {
      dates: [],
      equities: [],
      max_drawdown: 0,
      max_drawdown_duration: 0,
      current_drawdown: 0,
      peak: 0,
      series: [],
    }
  }
  if (path === '/api/risk/rolling') return { dates: [], sharpe: [], volatility: [], var_95: [] }

  if (path === '/api/scanner/profiles') return []
  if (path === '/api/scanner/results') return []
  if (path === '/api/scanner/conditions') return []
  if (path.startsWith('/api/scanner/scan')) return { matches: [], scanned_count: 0, matched_count: 0 }

  if (path === '/api/research-platform/datasets') return []
  if (path === '/api/research-platform/training-runs') return []
  if (path === '/api/research/model/train') return {}
  if (path.startsWith('/api/trend-research/factors/')) return { rows: [] }
  if (path === '/api/trend-research/config' && method === 'PUT') return { ok: true }
  if (path === '/api/trend-research/config') {
    return { whitelist: ['BTC-USDT-SWAP'], inst_type: 'SWAP', timeframe: '1H', bar_count: 500, enabled: false }
  }

  if (path === '/api/journal/entries') return []
  if (path === '/api/journal/tags') return []
  if (path === '/api/journal/stats') return { total_entries: 0, group_by: 'tag', groups: [] }

  if (path === '/api/assistant/status') return { enabled: true, configured: false }
  if (path === '/api/assistant/agent/tools') return []
  if (path === '/api/assistant/agent/sessions') {
    if (method === 'POST') return { id: 'session-1', title: '新会话', created_at: '2026-05-28T00:00:00.000Z' }
    return []
  }
  if (path.startsWith('/api/assistant/agent/sessions/')) return { session: {}, messages: [] }
  if (path === '/api/assistant/agent/patrol/status') return { running: false }
  if (path === '/api/assistant/agent/patrol/config') return { enabled: false }
  if (path === '/api/assistant/agent/order-drafts') return []

  return req?.body ?? {}
}

function watchedSymbol() {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    spot_inst_id: 'BTC-USDT',
    swap_inst_id: 'BTC-USDT-SWAP',
    sync_spot: false,
    sync_swap: true,
    sync_days: 30,
    sync_plans: [{ timeframe: '1H', enabled: true, bootstrap_days: 30, archive_mode: 'rolling' }],
    created_at: '2026-05-28T00:00:00.000Z',
    updated_at: '2026-05-28T00:00:00.000Z',
  }
}

function syncRuntimeConfig() {
  const settings = {
    max_sync_batches: 3,
    okx_page_pause_ms: 120,
    sync_job_concurrency: 2,
    window_fetch_concurrency: 2,
    window_fetch_batches_per_slice: 2,
    candle_upsert_transaction_chunk: 1000,
    okx_max_concurrency: 4,
    okx_public_rest_concurrency: 4,
    okx_private_rest_concurrency: 2,
    okx_trade_rest_concurrency: 2,
    okx_ws_control_concurrency: 2,
    okx_unknown_concurrency: 1,
  }
  return { settings, defaults: settings, limits: {}, active_sync_jobs: 0 }
}

function inventorySummary() {
  return {
    symbol_count: 1,
    managed_symbol_count: 1,
    managed_market_count: 1,
    watched_symbol_count: 1,
    watched_list_count: 1,
    watched_market_count: 1,
    orphan_symbol_count: 0,
    total_candles: 100,
    total_timeframe_records: 1,
    table_totals: { total: 100 },
  }
}

function inventoryRow() {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    managed: true,
    watched: true,
    orphan: false,
    candle_count: 100,
    timeframe_record_count: 1,
    storage_counts: { total: 100 },
    markets: {
      SWAP: {
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        managed: true,
        watched: true,
        timeframe_count: 1,
        candle_count: 100,
        gap_count: 0,
        history_complete_count: 1,
        timeframes: [],
      },
    },
  }
}

function guardianStatus() {
  return {
    enabled: true,
    active: false,
    policy_summary: '1H rolling',
    rolling_window_timeframes: ['1H'],
    full_backfill_timeframes: [],
    watched_count: 1,
    backfill_queue_size: 0,
    backfill_queue_preview: [],
    last_errors: [],
  }
}

function guardianConfig() {
  const settings = {
    enabled: true,
    scan_interval_seconds: 3600,
    max_full_backfill_jobs_per_cycle: 1,
    plans: [{ timeframe: '1H', enabled: true, bootstrap_days: 30, archive_mode: 'rolling' }],
  }
  return { settings, defaults: settings, status: guardianStatus() }
}

function tickCollectorStatus(overrides: Record<string, unknown> = {}) {
  return {
    running: false,
    active_symbols: [],
    book_channel: 'books5',
    total_trades_received: 0,
    total_bars_written: 0,
    last_trade_ts: 0,
    errors: [],
    ...overrides,
  }
}

function candle() {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    timestamp: 1_780_000_000_000,
    open: 100,
    high: 101,
    low: 99,
    close: 100,
    volume: 1,
  }
}

function ticker() {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    last: 100,
    ask: 100,
    bid: 100,
    open24h: 100,
    high24h: 101,
    low24h: 99,
    vol24h: 1000,
    change24h: 0,
    ts: 1_780_000_000_000,
  }
}

function orderbook() {
  return {
    inst_id: 'BTC-USDT-SWAP',
    bids: [{ price: 99, size: 1, count: 1 }],
    asks: [{ price: 101, size: 1, count: 1 }],
    ts: 1_780_000_000_000,
  }
}

function liveStatus() {
  return {
    status: 'stopped',
    running: false,
    run_id: 'run',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    symbol: 'BTC-USDT-SWAP',
    timeframe: '15m',
    inst_type: 'SWAP',
    mode: 'simulated',
    execution_mode: 'exchange_demo',
    risk_timeframe: '1m',
  }
}
