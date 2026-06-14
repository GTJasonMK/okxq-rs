import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  fetchGuardianConfig,
  fetchGuardianStatus,
  fetchInventory,
  fetchTickCollectorStatus,
  runDataGuardianNow,
  startTickCollector,
  stopTickCollector,
} from '@/api/market'

const invokeMock = vi.mocked(invoke)

describe('数据中心市场 API 归一化', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('库存接口在 API 层归一化 snake_case summary、rows 和市场字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        summary: {
          symbol_count: 2,
          managed_market_count: 3,
          watched_market_count: 2,
          total_candles: 120,
          table_totals: {
            candles: 120,
          },
        },
        rows: [
          {
            symbol: 'btc-usdt-swap',
            base_ccy: 'BTC',
            managed: true,
            watched: false,
            orphan: false,
            candle_count: 100,
            timeframe_record_count: 4,
            storage_counts: {
              candles: 100,
            },
            markets: {
              SWAP: {
                inst_id: 'BTC-USDT-SWAP',
                inst_type: 'SWAP',
                managed: true,
                watched: false,
                timeframe_count: 2,
                candle_count: 100,
                history_complete_count: 1,
                newest_time: '2026-05-28T00:00:00.000Z',
              },
            },
          },
        ],
      },
    })

    await expect(fetchInventory()).resolves.toMatchObject({
      summary: {
        symbol_count: 2,
        managed_market_count: 3,
        watched_market_count: 2,
        total_candles: 120,
        table_totals: {
          candles: 120,
        },
      },
      rows: [
        {
          symbol: 'BTC-USDT',
          base_ccy: 'BTC',
          managed: true,
          watched: false,
          orphan: false,
          candle_count: 100,
          timeframe_record_count: 4,
          storage_counts: {
            candles: 100,
          },
          markets: {
            SWAP: {
              inst_id: 'BTC-USDT-SWAP',
              inst_type: 'SWAP',
              managed: true,
              watched: false,
              timeframe_count: 2,
              candle_count: 100,
              history_complete_count: 1,
              newest_time: '2026-05-28T00:00:00.000Z',
            },
          },
        },
      ],
    })
  })

  it('秒级采集器状态和启停结果在 API 层归一化', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          running: false,
          active_symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
          book_channel: 'books50-l2-tbt',
          total_trades_received: 12,
          total_bars_written: 6,
          last_trade_ts: 1779926400000,
          errors: ['e1'],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          message: 'started',
          status: {
            running: true,
            active_symbols: ['BTC-USDT-SWAP'],
            total_trades_received: 1,
          },
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          message: 'stopped',
          status: {
            running: false,
            active_symbols: ['BTC-USDT-SWAP'],
            total_bars_written: 8,
          },
        },
      })

    await expect(fetchTickCollectorStatus()).resolves.toMatchObject({
      running: false,
      active_symbols: ['BTC-USDT-SWAP', 'ETH-USDT-SWAP'],
      book_channel: 'books50-l2-tbt',
      total_trades_received: 12,
      total_bars_written: 6,
      last_trade_ts: 1779926400000,
      errors: ['e1'],
    })

    await expect(startTickCollector()).resolves.toMatchObject({
      message: 'started',
      status: {
        running: true,
        active_symbols: ['BTC-USDT-SWAP'],
        total_trades_received: 1,
      },
    })

    await expect(stopTickCollector()).resolves.toMatchObject({
      message: 'stopped',
      status: {
        running: false,
        active_symbols: ['BTC-USDT-SWAP'],
        total_bars_written: 8,
      },
    })
  })

  it('Guardian 状态、配置和手动运行结果在 API 层归一化', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          enabled: false,
          active: true,
          policy_summary: '2 个周期',
          rolling_window_timeframes: ['1m', '15m'],
          full_backfill_timeframes: ['1D'],
          watched_count: 3,
          backfill_queue_preview: [
            {
              task_id: 'sync-1',
              inst_id: 'BTC-USDT-SWAP',
              inst_type: 'SWAP',
              timeframe: '1m',
              status: 'running',
              progress: 42,
            },
          ],
          last_errors: [{ message: 'err' }],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          settings: {
            enabled: true,
            scan_interval_seconds: 600,
            max_full_backfill_jobs_per_cycle: 2,
            plans: [
              {
                timeframe: '1H',
                enabled: false,
                bootstrap_days: 120,
                archive_mode: 'full',
              },
            ],
          },
          defaults: {
            enabled: true,
            plans: [],
          },
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          active: false,
          last_sync_results: [
            {
              task_id: 'sync-2',
              inst_id: 'ETH-USDT-SWAP',
              inst_type: 'SWAP',
              timeframe: '15m',
              status: 'queued',
            },
          ],
        },
      })

    await expect(fetchGuardianStatus()).resolves.toMatchObject({
      enabled: false,
      active: true,
      policy_summary: '2 个周期',
      rolling_window_timeframes: ['1m', '15m'],
      full_backfill_timeframes: ['1D'],
      watched_count: 3,
      backfill_queue_preview: [
        {
          task_id: 'sync-1',
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '1m',
          status: 'running',
          progress: 42,
        },
      ],
      last_errors: [{ message: 'err' }],
    })

    await expect(fetchGuardianConfig()).resolves.toMatchObject({
      settings: {
        enabled: true,
        scan_interval_seconds: 600,
        max_full_backfill_jobs_per_cycle: 2,
        plans: [
          {
            timeframe: '1H',
            enabled: false,
            bootstrap_days: 120,
            archive_mode: 'full',
          },
        ],
      },
    })

    await expect(runDataGuardianNow()).resolves.toMatchObject({
      active: false,
      last_sync_results: [
        {
          task_id: 'sync-2',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          status: 'queued',
        },
      ],
    })
  })
})
