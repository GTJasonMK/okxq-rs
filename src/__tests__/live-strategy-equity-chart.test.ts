import { describe, expect, it } from 'vitest'
import {
  equityHistory,
  equitySnapshot,
  liveOrder,
} from './fixtures/liveStrategy'
import {
  liveEquitySnapshotsForChart,
  liveOrdersForEquityChart,
} from '@/utils/liveStrategyEquityChart'
import { buildEquityCandles } from '@/utils/strategyExecution'

describe('live strategy equity chart adapter', () => {
  it('converts sorted live equity snapshots into equity candle snapshots with position detail', () => {
    const base = 1_780_000_000_000
    const rows = liveEquitySnapshotsForChart(equityHistory({
      count: 2,
      snapshots: [
        equitySnapshot({
          id: 2,
          timestamp: base + 60_000,
          equity: 1010,
          position_side: 'short',
          price: 95,
          entry_price: 100,
          quantity: 2,
          unrealized_pnl: 10,
        }),
        equitySnapshot({
          id: 1,
          timestamp: base,
          equity: 1000,
          position_side: 'flat',
          price: 100,
          quantity: 0,
          unrealized_pnl: 0,
        }),
      ],
    }))

    expect(rows.map(row => row.time)).toEqual([base, base + 60_000])
    expect(rows[0]).toMatchObject({
      equity: 1000,
      position_side: 'flat',
      position_notional: 0,
      positions: [],
    })
    expect(rows[1]).toMatchObject({
      equity: 1010,
      position_side: 'short',
      position_notional: 190,
      unrealized_pnl: 10,
    })
    expect(rows[1].positions?.[0]).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      side: 'short',
      entry_price: 100,
      quantity: 2,
      mark_price: 95,
      notional: 190,
      unrealized_pnl: 10,
      unrealized_pnl_pct: 5,
    })

    const candles = buildEquityCandles(rows, '15m')
    expect(candles.length).toBe(1)
    expect(candles[0]).toMatchObject({
      open: 1000,
      high: 1010,
      low: 1000,
      close: 1010,
      snapshot_count: 2,
    })
  })

  it('keeps aggregate portfolio positions when live side is multi', () => {
    const rows = liveEquitySnapshotsForChart(equityHistory({
      count: 1,
      snapshots: [
        equitySnapshot({
          position_side: 'multi',
          price: 120,
          entry_price: 100,
          quantity: 3,
          unrealized_pnl: 60,
          equity: 1200,
        }),
      ],
    }))

    expect(rows[0]).toMatchObject({
      position_side: 'portfolio',
      position_notional: 360,
    })
    expect(rows[0].positions?.[0]).toMatchObject({
      side: 'portfolio',
      unrealized_pnl: 60,
      unrealized_pnl_pct: 20,
    })
  })

  it('converts live orders into chart events without requiring realized pnl', () => {
    const base = 1_780_000_000_000
    const trades = liveOrdersForEquityChart([
      liveOrder({
        timestamp: base + 60_000,
        side: 'buy',
        action: 'close_position',
        px: 90,
        sz: 2,
      }),
      liveOrder({
        timestamp: base,
        side: 'sell',
        action: 'open_position',
        px: 100,
        sz: 2,
      }),
    ])

    expect(trades.map(trade => trade.timestamp)).toEqual([base, base + 60_000])
    expect(trades[0]).toMatchObject({
      action: 'open',
      side: 'sell',
      pos_side: 'short',
      price: 100,
      quantity: 2,
      value: 200,
    })
    expect(trades[1]).toMatchObject({
      action: 'close',
      side: 'buy',
      pos_side: 'short',
      exit_price: 90,
      value: 180,
      pnl: 0,
    })
  })

  it('converts live orders with actual fill economics before submitted economics', () => {
    const base = 1_780_000_000_000
    const trades = liveOrdersForEquityChart([
      liveOrder({
        timestamp: base,
        side: 'buy',
        action: 'open_position',
        px: 100,
        sz: 3,
        avg_fill_price: 106.6666666667,
        filled_size: 2,
        total_fee: -0.03,
      }),
    ])

    expect(trades).toHaveLength(1)
    expect(trades[0]).toMatchObject({
      price: 106.6666666667,
      quantity: 2,
      value: 213.3333333334,
      commission: 0.03,
    })
  })

  it('skips live orders whose economics are unknown', () => {
    const base = 1_780_000_000_000
    const trades = liveOrdersForEquityChart([
      liveOrder({
        timestamp: base,
        px: null,
        sz: 2,
        arrival_mid_px: null,
      }),
      liveOrder({
        timestamp: base + 60_000,
        px: 100,
        sz: null,
      }),
      liveOrder({
        timestamp: base + 120_000,
        px: 101,
        sz: 1,
      }),
    ])

    expect(trades).toHaveLength(1)
    expect(trades[0]).toMatchObject({
      timestamp: base + 120_000,
      price: 101,
      quantity: 1,
    })
  })
})
