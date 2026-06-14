import { describe, expect, it } from 'vitest'
import type { BacktestEquitySnapshot, BacktestPositionSnapshot, BacktestTrade } from '@/types'
import type { EquityCandle } from '@/utils/strategyExecution'
import {
  chartTimeSecond,
  equityCandleReturnPct,
  equityHistogramValues,
  equityHoverTooltip,
  equityLegend,
  equityPoints,
  equityRangeSummary,
  equityStats,
  tooltipEvent,
} from '@/utils/equityCandleChart'

describe('equityCandleChart utils', () => {
  it('derives legend and drawdown stats from equity snapshots', () => {
    const candles = [
      equityCandle({ timestamp: 60_000, open: 100, high: 115, low: 95, close: 110 }),
    ]
    const snapshots = [
      snapshot({ time: 60_000, equity: 100 }),
      snapshot({ time: 120_000, equity: 120 }),
      snapshot({ time: 180_000, equity: 90 }),
      snapshot({ time: 240_000, equity: 110 }),
    ]

    const stats = equityStats(snapshots, candles)

    expect(equityLegend(candles[0])).toMatchObject({
      open: '100',
      close: '110',
      change: '+10.00%',
      positive: true,
    })
    expect(equityCandleReturnPct(candles[0])).toBeCloseTo(10)
    expect(stats).toMatchObject({
      max: { timestamp: 120_000, value: 120 },
      min: { timestamp: 180_000, value: 90 },
      range: 30,
      maxDrawdownPct: 25,
    })
    expect(equityRangeSummary(stats!)).toMatchObject({
      range: '30',
      drawdown: '25.00%',
    })
  })

  it('falls back to candle OHLC points when snapshots are unavailable', () => {
    const points = equityPoints([], [
      equityCandle({ timestamp: 120_000, open: 100, high: 125, low: 95, close: 110 }),
    ])

    expect(points).toEqual([
      { timestamp: 120_000, value: 100 },
      { timestamp: 120_000, value: 125 },
      { timestamp: 120_000, value: 95 },
      { timestamp: 120_000, value: 110 },
    ])
    expect(equityStats([], [
      equityCandle({ timestamp: 120_000, open: 100, high: 125, low: 95, close: 110 }),
    ])).toMatchObject({
      max: { value: 125 },
      min: { value: 95 },
      range: 30,
    })
  })

  it('builds hover tooltip content from the candle bucket event first', () => {
    const tooltip = equityHoverTooltip({
      candle: equityCandle({ timestamp: 60_000, open: 100, close: 105 }),
      snapshots: [snapshot({
        time: 70_000,
        equity: 106,
        cash: 900,
        position_notional: 250,
        unrealized_pnl: 6,
        position_side: 'long',
        leverage: 1.5,
        positions: [
          position({
            symbol: 'BTC-USDT-SWAP',
            side: 'long',
            quantity: 2,
            entry_price: 100,
            mark_price: 103,
            entry_notional: 200,
            position_notional: 206,
            unrealized_pnl: 6,
          }),
          position({
            symbol: 'ETH-USDT-SWAP',
            side: 'short',
            quantity: 0.5,
            entry_price: 2000,
            mark_price: 1980,
            entry_notional: 1000,
            position_notional: 990,
            unrealized_pnl: 10,
          }),
        ],
      })],
      trades: [
        trade({ timestamp: 70_000, action: 'close', side: 'sell', pos_side: 'long', pnl: 12.5 }),
        trade({ timestamp: 180_000, action: 'open', side: 'buy', pos_side: 'long' }),
      ],
      timeframe: '1m',
      pointX: 50,
      pointY: 60,
      containerWidth: 480,
      containerHeight: 360,
    })

    expect(tooltip).toMatchObject({
      change: '+5.00%',
      equity: '106',
      cash: '900',
      notional: '250',
      unrealized: '+6',
      unrealizedClass: 'positive',
      position: '多',
      positionDetail: '多 · 250',
      positionClass: 'positive',
      exposure: '235.8%',
      leverage: '1.5x',
      eventTitle: '本K线事件',
    })
    expect(tooltip.events).toHaveLength(1)
    expect(tooltip.events[0]).toMatchObject({
      symbol: 'BTC',
      label: '平多',
      pnl: '+12.5',
      pnlClass: 'positive',
    })
    expect(tooltip.positions).toEqual([
      expect.objectContaining({
        symbol: 'BTC-USDT-SWAP',
        side: '多单',
        quantity: '2',
        entryPrice: '100',
        markPrice: '103',
        notional: '206',
        pnl: '+6',
        returnPct: '+3.00%',
      }),
      expect.objectContaining({
        symbol: 'ETH-USDT-SWAP',
        side: '空单',
        quantity: '0.5',
        entryPrice: '2,000',
        markPrice: '1,980',
        notional: '990',
        pnl: '+10',
        returnPct: '+1.00%',
      }),
    ])
  })

  it('shows unknown sparse position economics as placeholders instead of zeroes', () => {
    const tooltip = equityHoverTooltip({
      candle: equityCandle({ timestamp: 60_000, open: 1000, close: 1000 }),
      snapshots: [snapshot({
        time: 70_000,
        equity: 1000,
        cash: null,
        position_value: null,
        position_notional: null,
        unrealized_pnl: null,
        position_side: 'portfolio',
        positions: [
          position({
            entry_price: null,
            quantity: null,
            entry_timestamp: null,
            entry_notional: null,
            mark_price: null,
            notional: null,
            position_notional: null,
            unrealized_pnl: null,
            unrealized_pnl_pct: null,
          }),
        ],
      })],
      trades: [],
      timeframe: '1m',
      pointX: 50,
      pointY: 60,
      containerWidth: 480,
      containerHeight: 360,
    })

    expect(tooltip).toMatchObject({
      cash: '1,000',
      notional: '--',
      unrealized: '--',
      unrealizedClass: '',
      position: '组合',
      positionDetail: '组合 · --',
      exposure: '--',
    })
    expect(tooltip.positions[0]).toMatchObject({
      quantity: '--',
      entryPrice: '--',
      markPrice: '--',
      notional: '--',
      pnl: '--',
      pnlClass: '',
      returnPct: '--',
      returnClass: '',
    })
  })

  it('uses aggregate exposure as a fallback position when per-position snapshots are missing', () => {
    const tooltip = equityHoverTooltip({
      candle: equityCandle({ timestamp: 60_000, open: 1000, close: 1008 }),
      snapshots: [snapshot({
        time: 70_000,
        equity: 1008,
        cash: 990,
        position_notional: 420,
        unrealized_pnl: 18,
        position_side: 'long',
      })],
      trades: [],
      timeframe: '1m',
      pointX: 50,
      pointY: 60,
      containerWidth: 480,
      containerHeight: 360,
    })

    expect(tooltip.positionsTotal).toBe(1)
    expect(tooltip.positions[0]).toMatchObject({
      symbol: '组合持仓',
      side: '多单',
      notional: '420',
      pnl: '+18',
    })
  })

  it('shows the latest in-bucket position when the candle closes flat', () => {
    const tooltip = equityHoverTooltip({
      candle: equityCandle({ timestamp: 60_000, open: 1000, close: 1008 }),
      snapshots: [
        snapshot({
          time: 70_000,
          equity: 1004,
          cash: 900,
          position_notional: 250,
          unrealized_pnl: 6,
          position_side: 'short',
          positions: [
            position({
              symbol: 'ARB-USDT-SWAP',
              side: 'short',
              quantity: 120,
              entry_price: 0.103,
              mark_price: 0.101,
              entry_notional: 12.36,
              position_notional: 12.12,
              unrealized_pnl: 0.24,
            }),
          ],
        }),
        snapshot({
          time: 110_000,
          equity: 1008,
          cash: 1008,
          position_notional: 0,
          unrealized_pnl: 0,
          position_side: 'flat',
          positions: undefined,
        }),
      ],
      trades: [],
      timeframe: '1m',
      pointX: 50,
      pointY: 60,
      containerWidth: 480,
      containerHeight: 360,
    })

    expect(tooltip).toMatchObject({
      equity: '1,008',
      positionTitle: 'K线内持仓',
      position: '空',
      positionDetail: '空 · 250',
      notional: '250',
      unrealized: '+6',
    })
    expect(tooltip.positionsTotal).toBe(1)
    expect(tooltip.positions[0]).toMatchObject({
      symbol: 'ARB-USDT-SWAP',
      side: '空单',
      quantity: '120',
      entryPrice: '0.103',
      markPrice: '0.101',
      notional: '12.12',
      pnl: '+0.24',
    })
  })

  it('does not show nearby events when the hovered candle has no bucket trades', () => {
    const tooltip = equityHoverTooltip({
      candle: equityCandle({ timestamp: 300_000, open: 100, close: 105 }),
      snapshots: [],
      trades: [
        trade({ timestamp: 60_000, action: 'open', side: 'buy', pos_side: 'long' }),
        trade({ timestamp: 420_000, action: 'close', side: 'sell', pos_side: 'long', pnl: 4 }),
      ],
      timeframe: '1m',
      pointX: 50,
      pointY: 60,
      containerWidth: 480,
      containerHeight: 360,
    })

    expect(tooltip.eventTitle).toBe('本K线事件')
    expect(tooltip.events).toEqual([])
  })

  it('builds switchable histogram metrics without changing equity candles', () => {
    const candles = [
      equityCandle({ timestamp: 60_000, open: 100, high: 105, low: 95, close: 105 }),
      equityCandle({ timestamp: 120_000, open: 105, high: 106, low: 90, close: 94.5 }),
      equityCandle({ timestamp: 180_000, open: 94.5, high: 120, low: 94, close: 120 }),
    ]
    const snapshots = [
      snapshot({ time: 70_000, equity: 105, position_notional: 210, position_side: 'long' }),
      snapshot({ time: 130_000, equity: 94.5, position_notional: 47.25, position_side: 'short' }),
      snapshot({ time: 190_000, equity: 120, position_notional: 0, position_side: 'flat' }),
    ]

    const returnValues = equityHistogramValues({
      candles,
      snapshots,
      timeframe: '1m',
      metric: 'return_pct',
    }).map(point => point.value)
    expect(returnValues[0]).toBeCloseTo(5)
    expect(returnValues[1]).toBeCloseTo(-10)
    expect(returnValues[2]).toBeCloseTo(26.984126984126988)
    expect(equityHistogramValues({
      candles,
      snapshots,
      timeframe: '1m',
      metric: 'drawdown_pressure_pct',
    }).map(point => point.value)).toEqual([0, -10, 0])
    expect(equityHistogramValues({
      candles,
      snapshots,
      timeframe: '1m',
      metric: 'exposure_pct',
    })).toEqual([
      { timestamp: 60_000, value: 200, side: 'long' },
      { timestamp: 120_000, value: 50, side: 'short' },
      { timestamp: 180_000, value: 0, side: 'flat' },
    ])
  })

  it('converts lightweight chart times to unix seconds', () => {
    expect(chartTimeSecond({ year: 2026, month: 6, day: 5 })).toBe(1780617600)
  })

  it('formats individual tooltip events by trade action', () => {
    expect(tooltipEvent(
      trade({ timestamp: 60_000, action: 'open', side: 'sell', pos_side: 'short' }),
      0,
    )).toMatchObject({
      label: '开空',
      sideClass: 'negative',
      pnl: '--',
      pnlClass: '',
    })
    expect(tooltipEvent(
      trade({ timestamp: 120_000, action: 'close', side: 'buy', pos_side: 'short', pnl: -3 }),
      1,
    )).toMatchObject({
      label: '平空',
      sideClass: 'negative',
      pnl: '-3',
      pnlClass: 'negative',
    })
    expect(tooltipEvent(
      trade({ timestamp: 180_000, action: 'close', side: 'sell', pos_side: 'long', pnl: 4 }),
      2,
    )).toMatchObject({
      label: '平多',
      sideClass: 'positive',
      pnl: '+4',
      pnlClass: 'positive',
    })
    expect(tooltipEvent(
      trade({ timestamp: 240_000, action: 'funding', side: 'funding', pos_side: 'long', funding: -0.12 }),
      3,
    )).toMatchObject({
      label: '资金费',
      sideClass: 'negative',
      pnl: '-0.12',
      pnlClass: 'negative',
    })
  })
})

function equityCandle(overrides: Partial<EquityCandle> = {}): EquityCandle {
  return {
    timestamp: 60_000,
    open: 100,
    high: 100,
    low: 100,
    close: 100,
    volume: 1,
    snapshot_count: 1,
    ...overrides,
  }
}

function snapshot(overrides: Partial<BacktestEquitySnapshot> = {}): BacktestEquitySnapshot {
  return {
    time: 60_000,
    equity: 100,
    cash: 100,
    position_value: 0,
    position_notional: 0,
    unrealized_pnl: 0,
    position_side: 'flat',
    leverage: 1,
    ...overrides,
  }
}

function position(overrides: Partial<BacktestPositionSnapshot> = {}): BacktestPositionSnapshot {
  return {
    symbol: 'BTC-USDT-SWAP',
    side: 'long',
    inst_type: 'SWAP',
    timeframe: '1m',
    entry_price: 100,
    quantity: 1,
    entry_timestamp: 60_000,
    entry_notional: 100,
    entry_reason: 'open',
    reason: 'open',
    stop_loss: 0,
    take_profit: 0,
    planned_exit_time: 0,
    planned_exit_reason: '',
    planned_hold_bars: 0,
    mark_price: 100,
    notional: 100,
    position_notional: 100,
    unrealized_pnl: 0,
    unrealized_pnl_pct: 0,
    ...overrides,
  }
}

function trade(overrides: Partial<BacktestTrade> = {}): BacktestTrade {
  return {
    timestamp: 60_000,
    datetime: '',
    entry_time: '',
    exit_time: '',
    side: 'buy',
    action: 'open',
    pos_side: 'long',
    price: 100,
    entry_price: 100,
    exit_price: 0,
    quantity: 1,
    value: 100,
    commission: 0,
    pnl: 0,
    pnl_pct: 0,
    funding: 0,
    equity: 0,
    reason: '',
    symbol: 'BTC-USDT-SWAP',
    ...overrides,
  }
}
