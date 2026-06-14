import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  createAlert,
  deleteAlert,
  fetchAlerts,
  fetchCandles,
  fetchMarketGapPlan,
  fetchOrderbook,
  fetchRecentTrades,
  fetchSymbols,
  fetchSyncJobs,
  fetchSyncRecords,
  fetchTickers,
  fetchWatchedSymbols,
  repairWatchedSymbol,
  startGapRepairJob,
  updateAlert,
} from '@/api/market'

const invokeMock = vi.mocked(invoke)

describe('市场 API 契约归一化', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('fetchSymbols 返回市场标的元信息，不误 cast 为关注币种', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          symbol: 'btc-usdt',
          base_ccy: 'BTC',
          inst_id: 'btc-usdt-swap',
          inst_type: 'swap',
          timeframes: ['1m', '1h'],
          candle_count: 120,
          managed: true,
          watched: false,
        },
      ],
    })

    await expect(fetchSymbols()).resolves.toEqual([
      {
        symbol: 'BTC-USDT',
        base_ccy: 'BTC',
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframes: ['1m', '1H'],
        candle_count: 120,
        managed: true,
        watched: false,
      },
    ])
  })

  it('fetchAlerts 使用后端价格提醒 schema', async () => {
    invokeMock.mockResolvedValueOnce({
      success: true,
      data: [
        {
          id: 'pa_1',
          inst_id: 'eth-usdt-swap',
          inst_type: 'swap',
          alert_type: 'change',
          direction: 'below',
          change_percent: -5,
          enabled: false,
          trigger_once: false,
          cooldown_seconds: 60,
          created_at: '2026-05-28T00:00:00.000Z',
          triggered_at: '2026-05-28T01:00:00.000Z',
          last_value: -6,
          last_trigger_value: -5.5,
          last_trigger_ts: 1779926400000,
        },
      ],
    })

    await expect(fetchAlerts()).resolves.toMatchObject([
      {
        id: 'pa_1',
        inst_id: 'ETH-USDT-SWAP',
        inst_type: 'SWAP',
        alert_type: 'change',
        direction: 'below',
        change_percent: -5,
        enabled: false,
        trigger_once: false,
        cooldown_seconds: 60,
        last_value: -6,
        last_trigger_value: -5.5,
        last_trigger_ts: 1779926400000,
      },
    ])
  })

  it('关注币种和同步任务端点使用直接数组 payload', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            symbol: 'eth-usdt',
            base_ccy: 'ETH',
            spot_inst_id: 'ETH-USDT',
            swap_inst_id: 'ETH-USDT-SWAP',
            sync_spot: false,
            sync_swap: true,
            archive_all_history: false,
            sync_days: 365,
            sync_strategy: 'direct',
            sync_plans: [
              { timeframe: '1H', enabled: true, bootstrap_days: 365, archive_mode: 'rolling' },
            ],
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            task_id: 'sync_1',
            inst_id: 'ETH-USDT-SWAP',
            inst_type: 'SWAP',
            timeframe: '1m',
            source_timeframe: '1m',
            target_timeframes: ['1m', '1H'],
            mode: 'window',
            status: 'running',
            progress: 42,
            reused_existing: false,
            history_complete: false,
            created_at: '2026-05-28T00:00:00.000Z',
            updated_at: '2026-05-28T00:01:00.000Z',
          },
        ],
      })

    const watched = await fetchWatchedSymbols()
    expect(watched).toMatchObject([{
      symbol: 'ETH-USDT',
      sync_spot: false,
      sync_swap: true,
      sync_days: 365,
      sync_plans: [{ timeframe: '1H', enabled: true, bootstrap_days: 365 }],
    }])
    expect(watched[0]).not.toHaveProperty('sync_strategy')
    await expect(fetchSyncJobs()).resolves.toMatchObject([
      {
        task_id: 'sync_1',
        inst_id: 'ETH-USDT-SWAP',
        target_timeframes: ['1m', '1H'],
        progress: 42,
        reused_existing: false,
      },
    ])
  })

  it('市场列表端点不读取旧 wrapper 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: { symbols: [{ inst_id: 'BTC-USDT-SWAP', inst_type: 'SWAP' }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { watched_symbols: [{ symbol: 'BTC-USDT' }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { candles: [{ timestamp: 1, open: 1, high: 1, low: 1, close: 1, volume: 1 }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { tickers: [{ inst_id: 'BTC-USDT-SWAP', last: 1 }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { trades: [{ trade_id: '1', side: 'sell' }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { alerts: [{ id: 'pa_1' }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { jobs: [{ task_id: 'sync_1' }] },
      })

    await expect(fetchSymbols()).resolves.toEqual([])
    await expect(fetchWatchedSymbols()).resolves.toEqual([])
    await expect(fetchCandles('BTC-USDT-SWAP')).resolves.toEqual([])
    await expect(fetchTickers()).resolves.toEqual([])
    await expect(fetchRecentTrades('BTC-USDT-SWAP')).resolves.toEqual([])
    await expect(fetchAlerts()).resolves.toEqual([])
    await expect(fetchSyncJobs()).resolves.toEqual([])
  })

  it('fetchSyncJobs 使用数组 task_ids，不发送逗号拼接字符串', async () => {
    invokeMock.mockResolvedValueOnce({ code: 0, data: [] })

    await expect(fetchSyncJobs({ task_ids: ['sync_1', 'sync_2'], limit: 2 })).resolves.toEqual([])

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/market/sync/jobs',
        params: {
          task_ids: ['sync_1', 'sync_2'],
          limit: 2,
        },
        body: undefined,
      },
    })
  })

  it('fetchSyncRecords 使用轻量同步记录端点并归一化记录字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          inst_id: 'eth-usdt-swap',
          inst_type: 'swap',
          timeframe: '1h',
          last_sync_time: '2026-06-01T00:00:00Z',
          oldest_timestamp: 1777593600000,
          newest_timestamp: 1777680000000,
          oldest_time: '2026-05-01T00:00:00Z',
          newest_time: '2026-05-02T00:00:00Z',
          candle_count: 24,
          expected_candle_count: 25,
          gap_count: 1,
          coverage_ratio: 0.96,
          history_complete: true,
          last_sync_mode: 'window',
        },
        {
          inst_id: 'BTC-USDT',
          inst_type: 'SPOT',
          timeframe: 'bad',
        },
      ],
    })

    await expect(fetchSyncRecords({ watched_only: true })).resolves.toEqual([
      {
        inst_id: 'ETH-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '1H',
        last_sync_time: '2026-06-01T00:00:00Z',
        oldest_timestamp: 1777593600000,
        newest_timestamp: 1777680000000,
        oldest_time: '2026-05-01T00:00:00Z',
        newest_time: '2026-05-02T00:00:00Z',
        candle_count: 24,
        expected_candle_count: 25,
        gap_count: 1,
        coverage_ratio: 0.96,
        history_complete: true,
        last_sync_mode: 'window',
      },
    ])

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/market/sync/records',
        params: { watched_only: true },
        body: undefined,
      },
    })
  })

  it('repairWatchedSymbol 复用关注币种保存结果的严格数值归一化', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        symbol: 'BTC-USDT',
        requested_markets: { spot: true, swap: true },
        effective_markets: { spot: false, swap: true },
        sync_jobs: [
          {
            task_id: 'sync_repair_1',
            inst_id: 'btc-usdt-swap',
            inst_type: 'swap',
            timeframe: '1m',
            progress: '75',
          },
        ],
        started_count: '1',
        reused_count: '0',
        exact_gap_jobs: '2',
        rule_jobs: 'bad',
      },
    })

    await expect(repairWatchedSymbol('BTC-USDT')).resolves.toMatchObject({
      sync_jobs: [
        {
          task_id: 'sync_repair_1',
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          progress: 0,
        },
      ],
      started_count: 0,
      reused_count: 0,
      exact_gap_jobs: 0,
      rule_jobs: 0,
    })
  })

  it('fetchMarketGapPlan 使用后端缺口计划合同并归一化补齐方式', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        inst_id: 'btc-usdt-swap',
        inst_type: 'swap',
        timeframe: '1m',
        source_timeframe: '1m',
        target_timeframes: ['1m'],
        range: {
          start_ts: 1704067200000,
          end_ts: 1704758340000,
          start_time: '2024-01-01T00:00:00Z',
          end_time: '2024-01-08T23:59:00Z',
        },
        local_range: {
          oldest_timestamp: null,
          newest_timestamp: null,
          oldest_time: null,
          newest_time: null,
        },
        expected_candles: 11520,
        available_candles: 0,
        missing_candles: 11520,
        coverage_ratio: 0,
        gap_event_count: 1,
        returned_gap_count: 1,
        returned_missing_candles: 11520,
        truncated: false,
        max_internal_gap_ms: 0,
        methods: {
          paginated_ranges: 0,
          historical_zip_ranges: 1,
        },
        gaps: [
          {
            start_ts: 1704067200000,
            end_ts: 1704758340000,
            start_time: '2024-01-01T00:00:00Z',
            end_time: '2024-01-08T23:59:00Z',
            span_ms: 691200000,
            missing_candles: 11520,
            method: 'historical_zip',
            reason: '大跨度历史缺口优先用 OKX 历史 zip 导入，避免大量分页请求',
            fetch_timeframe: '1m',
            target_timeframes: ['1m'],
            requires_derivation: false,
            zip: {
              provider: 'okx_historical_market_data',
              module: 'candlesticks',
              date_aggr_type: 'daily',
              source_timeframe: '1m',
            },
          },
        ],
      },
    })

    await expect(fetchMarketGapPlan({
      inst_id: 'btc-usdt-swap',
      inst_type: 'SWAP',
      timeframe: '1m',
      start_ts: 1704067200000,
      end_ts: 1704758340000,
    })).resolves.toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1m',
      missing_candles: 11520,
      methods: {
        historical_zip_ranges: 1,
      },
      gaps: [
        {
          method: 'historical_zip',
          missing_candles: 11520,
          zip: {
            date_aggr_type: 'daily',
            source_timeframe: '1m',
          },
        },
      ],
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/market/gaps/plan',
        params: undefined,
        body: {
          inst_id: 'btc-usdt-swap',
          inst_type: 'SWAP',
          timeframe: '1m',
          start_ts: 1704067200000,
          end_ts: 1704758340000,
        },
      },
    })
  })

  it('startGapRepairJob 启动后台缺口补齐任务并保留范围字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        task_id: 'sync_gap_001',
        inst_id: 'btc-usdt-swap',
        inst_type: 'swap',
        timeframe: '1h',
        source_timeframe: '1m',
        target_timeframes: ['1h'],
        mode: 'gap_repair',
        days: 1,
        start_ts: 1704067200000,
        end_ts: 1704150000000,
        repair_method: 'historical_zip',
        status: 'queued',
        progress: 0,
        message: '等待开始',
        target_fetch_count: 1440,
        target_save_count: 1440,
        target_derive_count: 24,
        target_batches: 1,
        reused_existing: true,
        created_at: '2024-01-01T00:00:00Z',
      },
    })

    await expect(startGapRepairJob({
      inst_id: 'btc-usdt-swap',
      inst_type: 'SWAP',
      timeframe: '1H',
      start_ts: 1704067200000,
      end_ts: 1704150000000,
      method: 'historical_zip',
    })).resolves.toMatchObject({
      task_id: 'sync_gap_001',
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      source_timeframe: '1m',
      target_timeframes: ['1H'],
      mode: 'gap_repair',
      start_ts: 1704067200000,
      end_ts: 1704150000000,
      repair_method: 'historical_zip',
      reused_existing: true,
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/market/gaps/repair/jobs',
        params: undefined,
        body: {
          inst_id: 'btc-usdt-swap',
          inst_type: 'SWAP',
          timeframe: '1H',
          start_ts: 1704067200000,
          end_ts: 1704150000000,
          method: 'historical_zip',
        },
      },
    })
  })

  it('createAlert/updateAlert 只发送后端规范字段并归一化返回', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          id: 'pa_2',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          alert_type: 'price',
          direction: 'above',
          target_price: 3000,
          enabled: false,
          trigger_once: false,
          cooldown_seconds: 120,
          created_at: '2026-05-28T00:00:00.000Z',
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          id: 'pa_2',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          alert_type: 'price',
          direction: 'below',
          target_price: 2800,
          enabled: true,
          trigger_once: true,
          cooldown_seconds: 300,
        },
      })

    await expect(createAlert({
      inst_id: 'eth-usdt-swap',
      alert_type: 'price',
      direction: 'above',
      target_price: 3000,
      enabled: false,
      trigger_once: false,
      cooldown_seconds: 120,
      note: '  watch  ',
    })).resolves.toMatchObject({
      id: 'pa_2',
      inst_id: 'ETH-USDT-SWAP',
      alert_type: 'price',
      direction: 'above',
      target_price: 3000,
      enabled: false,
      trigger_once: false,
    })

    await expect(updateAlert('pa_2', {
      direction: 'below',
      target_price: 2800,
      enabled: true,
    })).resolves.toMatchObject({
      id: 'pa_2',
      direction: 'below',
      target_price: 2800,
      enabled: true,
    })
    await deleteAlert('pa/2')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'local_api_request', {
      req: {
        method: 'POST',
        path: '/api/market/alerts',
        params: undefined,
        body: {
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          alert_type: 'price',
          direction: 'above',
          target_price: 3000,
          note: 'watch',
          enabled: false,
          trigger_once: false,
          cooldown_seconds: 120,
        },
      },
    })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'local_api_request', {
      req: {
        method: 'PATCH',
        path: '/api/market/alerts/pa_2',
        params: undefined,
        body: {
          direction: 'below',
          target_price: 2800,
          enabled: true,
        },
      },
    })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'local_api_request', {
      req: {
        method: 'DELETE',
        path: '/api/market/alerts/pa%2F2',
        params: undefined,
        body: undefined,
      },
    })
  })

  it('createAlert 不把非字符串或字符串数字转换为后端字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        id: 'pa_3',
        inst_id: 'ETH-USDT-SWAP',
        inst_type: 'SWAP',
        alert_type: 'price',
        direction: 'above',
        target_price: null,
        enabled: true,
        trigger_once: true,
        cooldown_seconds: 300,
      },
    })

    await createAlert({
      inst_id: 123,
      inst_type: 456,
      symbol: 789,
      alert_type: 'price',
      direction: 'above',
      target_price: '3000',
      change_percent: '5',
      note: 123,
      enabled: 'true',
      trigger_once: 'false',
      cooldown_seconds: '120',
      created_at: 1779926400000,
      updated_at: 1779926400000,
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/market/alerts',
        params: undefined,
        body: {
          alert_type: 'price',
          direction: 'above',
        },
      },
    })
  })

  it('fetchCandles 丢弃缺失或非正时间戳的 K 线', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '1m',
          timestamp: 0,
          open: 90,
          high: 91,
          low: 89,
          close: 90,
          volume: 1,
        },
        {
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '1m',
          timestamp: 1_700_000_000_000,
          open: 100,
          high: 101,
          low: 99,
          close: 100,
          volume: 2,
        },
      ],
    })

    await expect(fetchCandles('BTC-USDT-SWAP')).resolves.toMatchObject([
      { timestamp: 1_700_000_000_000, open: 100, volume: 2 },
    ])
  })

  it('fetchTickers 不把未知或非正 last 伪造成 0 行情', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        { inst_id: 'BTC-USDT-SWAP', inst_type: 'SWAP', last: null, open24h: 70000 },
        { inst_id: 'SOL-USDT-SWAP', inst_type: 'SWAP', last: 0, open24h: 120 },
        { inst_id: 'ETH-USDT-SWAP', inst_type: 'SWAP', last: 3600, open24h: 3500 },
      ],
    })

    await expect(fetchTickers()).resolves.toEqual([
      expect.objectContaining({ inst_id: 'ETH-USDT-SWAP', last: 3600 }),
    ])
  })

  it('fetchRecentTrades 不返回价格或数量未知的成交', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        { trade_id: 'bad-price', inst_id: 'BTC-USDT-SWAP', price: null, size: 1, side: 'buy', ts: 1 },
        { trade_id: 'bad-size', inst_id: 'BTC-USDT-SWAP', price: 70000, size: 0, side: 'sell', ts: 2 },
        { trade_id: 'ok', inst_id: 'BTC-USDT-SWAP', price: 70001, size: 0.1, side: 'sell', ts: 3 },
      ],
    })

    await expect(fetchRecentTrades('BTC-USDT-SWAP')).resolves.toEqual([
      expect.objectContaining({ trade_id: 'ok', price: 70001, size: 0.1 }),
    ])
  })

  it('fetchOrderbook 过滤未知或非正价格数量档位', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        inst_id: 'BTC-USDT-SWAP',
        bids: [
          { price: null, size: 1, count: 1 },
          { price: 70000, size: 0, count: 1 },
          { price: 69999, size: 2, count: 1 },
        ],
        asks: [
          { price: 70001, size: 1, count: 1 },
          { price: 70002, size: null, count: 1 },
        ],
        ts: 1,
      },
    })

    await expect(fetchOrderbook('BTC-USDT-SWAP')).resolves.toMatchObject({
      bids: [{ price: 69999, size: 2, count: 1 }],
      asks: [{ price: 70001, size: 1, count: 1 }],
    })
  })
})
