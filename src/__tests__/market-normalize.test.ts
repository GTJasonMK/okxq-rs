import { describe, expect, it } from 'vitest'
import {
  normalizeCandle,
  normalizeOrderbook,
  normalizeRecentTrade,
  normalizeSyncJob,
  normalizeTicker,
  normalizeWatchMutationResult,
  normalizeWatchedSymbol,
} from '@/api/marketNormalize'

describe('市场 API 归一化', () => {
  it('K 线归一化只接受后端 snake_case 字段', () => {
    expect(normalizeCandle({
      inst_id: 'btc-usdt-swap',
      inst_type: 'swap',
      timeframe: '1h',
      timestamp: 1779926400000,
      open: 100,
      high: 110,
      low: 90,
      close: 105,
      volume: 12.5,
      volume_ccy: 1250,
      volume_quote: 1251,
    })).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '1H',
      timestamp: 1779926400000,
      open: 100,
      high: 110,
      low: 90,
      close: 105,
      volume: 12.5,
      volume_ccy: 1250,
      volume_quote: 1251,
    })
  })

  it('Ticker 归一化只接受后端 snake_case 字段并推导涨跌幅', () => {
    expect(normalizeTicker({
      inst_id: 'eth-usdt-swap',
      inst_type: 'swap',
      last: 110,
      ask: 111,
      bid: 109,
      open24h: 100,
      high24h: 120,
      low24h: 95,
      vol24h: 3000,
      ts: 1779930000000,
    })).toMatchObject({
      inst_id: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      last: 110,
      ask: 111,
      bid: 109,
      open24h: 100,
      high24h: 120,
      low24h: 95,
      vol24h: 3000,
      change24h: 10,
    })
  })

  it('盘口归一化只接受后端标准档位对象', () => {
    expect(normalizeOrderbook({
      inst_id: 'btc-usdt-swap',
      bids: [{ price: 100, size: 2, count: 4 }],
      asks: [{ price: 101, size: 3, count: 5 }],
      ts: 1_700_000_000_000,
    })).toEqual({
      inst_id: 'BTC-USDT-SWAP',
      bids: [{ price: 100, size: 2, count: 4 }],
      asks: [{ price: 101, size: 3, count: 5 }],
      ts: 1_700_000_000_000,
    })
  })

  it('逐笔成交归一化只接受后端 snake_case 字段', () => {
    expect(normalizeRecentTrade({
      inst_id: 'sol-usdt-swap',
      trade_id: '12345',
      price: 155.5,
      size: 8,
      side: 'sell',
      ts: 1_700_000_000_000,
    })).toEqual({
      inst_id: 'SOL-USDT-SWAP',
      trade_id: '12345',
      price: 155.5,
      size: 8,
      side: 'sell',
      ts: 1_700_000_000_000,
    })
  })

  it('逐笔成交方向不解析大小写或交易方向别名', () => {
    expect(normalizeRecentTrade({ price: 1, size: 1, side: 'SELL' })).toMatchObject({ side: 'buy' })
    expect(normalizeRecentTrade({ price: 1, size: 1, side: 'short' })).toMatchObject({ side: 'buy' })
    expect(normalizeRecentTrade({ price: 1, size: 1, side: 'ask' })).toMatchObject({ side: 'buy' })
  })

  it('关注币种和同步状态只接受标准 JSON 类型', () => {
    const watched = normalizeWatchedSymbol({
      symbol: 'btc-usdt',
      sync_spot: false,
      sync_swap: false,
      archive_all_history: false,
      sync_days: 120,
      sync_strategy: 'direct',
      sync_plans: [
        { timeframe: '1H', enabled: false, bootstrap_days: 120, archive_mode: 'full' },
      ],
    })

    expect(watched.sync_spot).toBe(false)
    expect(watched.sync_swap).toBe(false)
    expect(watched.archive_all_history).toBe(false)
    expect(watched).not.toHaveProperty('sync_strategy')
    expect(watched.sync_plans).toHaveLength(1)
    expect(watched.sync_plans?.[0]).toMatchObject({
      timeframe: '1H',
      enabled: false,
      bootstrap_days: 120,
      archive_mode: 'full',
    })

    expect(normalizeSyncJob({
      task_id: 'job-1',
      inst_id: 'btc-usdt-swap',
      inst_type: 'swap',
      timeframe: '1m',
      progress: 10,
      reused_existing: false,
      history_complete: false,
    })).toMatchObject({
      reused_existing: false,
      history_complete: false,
    })
  })

  it('关注币种保存结果只接受标准 snake_case 任务和统计字段', () => {
    const result = normalizeWatchMutationResult({
      watched_symbol: {
        symbol: 'eth-usdt',
        sync_spot: false,
        sync_swap: true,
      },
      sync_jobs: [
        {
          task_id: 'job-1',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'swap',
          timeframe: '1m',
          status: 'queued',
          progress: 0,
        },
      ],
      cancelled_disabled_jobs: [
        {
          task_id: 'job-2',
          inst_id: 'ETH-USDT',
          inst_type: 'spot',
          timeframe: '1m',
          status: 'cancelled',
          progress: 100,
        },
      ],
      existed: false,
      started_count: 1,
      reused_count: 0,
      exact_gap_jobs: 1,
      rule_jobs: 0,
    })

    expect(result.existed).toBe(false)
    expect(result.started_count).toBe(1)
    expect(result.reused_count).toBe(0)
    expect(result.exact_gap_jobs).toBe(1)
    expect(result.rule_jobs).toBe(0)
    expect(result.watched_symbol).toMatchObject({
      symbol: 'ETH-USDT',
      sync_spot: false,
      sync_swap: true,
    })
    expect(result.sync_jobs[0]).toMatchObject({
      task_id: 'job-1',
      inst_id: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
    })
    expect(result.cancelled_disabled_jobs[0]).toMatchObject({
      task_id: 'job-2',
      inst_id: 'ETH-USDT',
      inst_type: 'SPOT',
    })
  })
})
