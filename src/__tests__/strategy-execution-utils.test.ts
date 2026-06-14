import { describe, expect, it } from 'vitest'
import type { BacktestEquitySnapshot, BacktestPositionSnapshot, BacktestTrade, Candle } from '@/types'
import {
  buildEquityCandles,
  equityCandleEnd,
  nearbyTradeEvents,
  nearbyTradeEventsSorted,
  selectedCandleAtOrBefore,
  selectedCandleAtOrBeforeSorted,
  selectedPositionSnapshotForEquityCandle,
  selectedSnapshotForEquityCandle,
  selectedSnapshotAtOrBefore,
  selectedSnapshotAtOrBeforeSorted,
  sortedCandles,
  sortedEquitySnapshots,
  sortedTradeEvents,
  tradeEventsForEquityCandle,
} from '@/utils/strategyExecution'

describe('strategy execution utilities', () => {
  it('aggregates equity snapshots into OHLC candles', () => {
    const snapshots: BacktestEquitySnapshot[] = [
      snapshot(60_000, 100),
      snapshot(120_000, 105),
      snapshot(3_660_000, 102),
      snapshot(3_900_000, 110),
    ]

    expect(buildEquityCandles(snapshots, '1H')).toEqual([
      {
        timestamp: 0,
        open: 100,
        high: 105,
        low: 100,
        close: 105,
        volume: 2,
        snapshot_count: 2,
      },
      {
        timestamp: 3_600_000,
        open: 102,
        high: 110,
        low: 102,
        close: 110,
        volume: 2,
        snapshot_count: 2,
      },
    ])
  })

  it('selects the latest snapshot and candle at or before a timestamp', () => {
    const snapshots = [snapshot(1000, 100), snapshot(3000, 103), snapshot(5000, 99)]
    const candles: Candle[] = [
      candle(1000, 10),
      candle(3000, 11),
      candle(5000, 12),
    ]

    expect(selectedSnapshotAtOrBefore(snapshots, 3500)?.equity).toBe(103)
    expect(selectedCandleAtOrBefore(candles, 3500)?.close).toBe(11)
  })

  it('ignores invalid chart rows when selecting details by timestamp', () => {
    const snapshots = [
      snapshot(0, 80),
      snapshot(500, Number.NaN),
      snapshot(1000, 100),
      snapshot(3000, 103),
    ]
    const candles: Candle[] = [
      candle(0, 8),
      candle(1000, 10),
      candle(3000, 11),
    ]

    expect(selectedSnapshotAtOrBefore(snapshots, 750)?.equity).toBe(100)
    expect(selectedSnapshotAtOrBefore(snapshots, 0)?.equity).toBe(103)
    expect(selectedCandleAtOrBefore(candles, 750)?.close).toBe(10)
    expect(selectedCandleAtOrBefore(candles, 0)?.close).toBe(11)
  })

  it('sorts nearby trade events by distance then returns chronological rows', () => {
    const rows = nearbyTradeEvents([
      trade(1000, 'open'),
      trade(5000, 'close'),
      trade(3000, 'open'),
    ], 3200, 2)

    expect(rows.map(row => row.timestamp)).toEqual([3000, 5000])
  })

  it('shows latest valid trade events when no timestamp is selected', () => {
    const rows = nearbyTradeEvents([
      trade(1000, 'open'),
      trade(5000, 'close'),
      trade(0, 'open'),
      trade(3000, 'open'),
    ], 0, 2)

    expect(rows.map(row => row.timestamp)).toEqual([5000, 3000])
  })

  it('supports pre-sorted rows for fast hover lookups without mutating source arrays', () => {
    const snapshots = [snapshot(3000, 103), snapshot(0, 80), snapshot(1000, 100)]
    const candles: Candle[] = [candle(3000, 11), candle(1000, 10)]
    const trades = [trade(5000, 'close'), trade(1000, 'open'), trade(3000, 'open')]

    const sortedSnapshots = sortedEquitySnapshots(snapshots)
    const sortedPriceCandles = sortedCandles(candles)
    const sortedTrades = sortedTradeEvents(trades)

    expect(snapshots.map(row => row.time)).toEqual([3000, 0, 1000])
    expect(candles.map(row => row.timestamp)).toEqual([3000, 1000])
    expect(trades.map(row => row.timestamp)).toEqual([5000, 1000, 3000])
    expect(sortedSnapshots.map(row => row.time)).toEqual([1000, 3000])
    expect(sortedPriceCandles.map(row => row.timestamp)).toEqual([1000, 3000])
    expect(sortedTrades.map(row => row.timestamp)).toEqual([1000, 3000, 5000])

    expect(selectedSnapshotAtOrBeforeSorted(sortedSnapshots, 3500)?.equity).toBe(103)
    expect(selectedCandleAtOrBeforeSorted(sortedPriceCandles, 3500)?.close).toBe(11)
    expect(nearbyTradeEventsSorted(sortedTrades, 3200, 2).map(row => row.timestamp)).toEqual([3000, 5000])
  })

  it('maps a hovered equity candle to its closing account snapshot and in-bucket events', () => {
    const start = Date.parse('2026-06-04T08:00:00.000Z')
    const snapshots = sortedEquitySnapshots([
      snapshot(start - 60_000, 990),
      snapshot(start + 15 * 60_000, 1000),
      snapshot(start + 55 * 60_000, 1012),
      snapshot(start + 65 * 60_000, 1008),
    ])
    const trades = sortedTradeEvents([
      trade(start - 60_000, 'open'),
      trade(start + 10 * 60_000, 'open'),
      trade(start + 45 * 60_000, 'close'),
      trade(start + 60 * 60_000, 'close'),
    ])

    expect(equityCandleEnd(start, '1H')).toBe(start + 60 * 60_000)
    expect(selectedSnapshotForEquityCandle(snapshots, start, '1H')?.equity).toBe(1012)
    expect(tradeEventsForEquityCandle(trades, start, '1H').map(row => row.timestamp)).toEqual([
      start + 10 * 60_000,
      start + 45 * 60_000,
    ])
  })

  it('uses the latest in-bucket position snapshot when the closing account snapshot is flat', () => {
    const start = Date.parse('2026-06-04T08:00:00.000Z')
    const snapshots = sortedEquitySnapshots([
      snapshot(start + 10 * 60_000, 1000, {
        position_side: 'short',
        position_notional: 240,
        positions: [
          position({
            symbol: 'ARB-USDT-SWAP',
            side: 'short',
            quantity: 100,
            entry_price: 0.1,
            mark_price: 0.099,
            position_notional: 9.9,
            unrealized_pnl: 1,
          }),
        ],
      }),
      snapshot(start + 55 * 60_000, 1005, {
        position_side: 'flat',
        position_notional: 0,
        positions: undefined,
      }),
    ])

    expect(selectedSnapshotForEquityCandle(snapshots, start, '1H')?.position_side).toBe('flat')
    expect(selectedPositionSnapshotForEquityCandle(snapshots, start, '1H')).toMatchObject({
      position_side: 'short',
      positions: [
        expect.objectContaining({ symbol: 'ARB-USDT-SWAP' }),
      ],
    })
  })
})

function snapshot(
  time: number,
  equity: number,
  overrides: Partial<BacktestEquitySnapshot> = {},
): BacktestEquitySnapshot {
  return {
    time,
    equity,
    cash: equity,
    position_value: 0,
    position_notional: 0,
    unrealized_pnl: 0,
    position_side: 'flat',
    leverage: 1,
    ...overrides,
  }
}

function position(overrides: Partial<BacktestPositionSnapshot>): BacktestPositionSnapshot {
  return {
    symbol: 'BTC-USDT-SWAP',
    side: 'long',
    inst_type: 'SWAP',
    timeframe: '15m',
    entry_price: 100,
    quantity: 1,
    entry_timestamp: 1,
    entry_notional: 100,
    entry_reason: '',
    reason: '',
    stop_loss: null,
    take_profit: null,
    planned_exit_time: null,
    planned_exit_reason: '',
    planned_hold_bars: null,
    mark_price: 100,
    notional: 100,
    position_notional: 100,
    unrealized_pnl: 0,
    unrealized_pnl_pct: 0,
    ...overrides,
  }
}

function candle(timestamp: number, close: number): Candle {
  return {
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '1H',
    timestamp,
    open: close,
    high: close,
    low: close,
    close,
    volume: 1,
  }
}

function trade(timestamp: number, action: string): BacktestTrade {
  return {
    timestamp,
    datetime: '',
    entry_time: '',
    exit_time: '',
    side: 'buy',
    action,
    pos_side: 'long',
    price: 1,
    entry_price: 1,
    exit_price: 1,
    quantity: 1,
    value: 1,
    commission: 0,
    pnl: 0,
    pnl_pct: 0,
    funding: 0,
    equity: 0,
    reason: '',
  }
}
