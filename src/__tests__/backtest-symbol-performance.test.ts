import { describe, expect, it } from 'vitest'
import type { BacktestTrade } from '@/types'
import { buildBacktestSymbolPerformance } from '@/utils/backtestSymbolPerformance'

describe('回测币种收益聚合', () => {
  it('按平仓事件汇总各标的已实现收益和贡献率', () => {
    const rows = buildBacktestSymbolPerformance([
      trade({ symbol: 'BTC-USDT-SWAP', action: 'open', side: 'buy', pos_side: 'long', value: 100, commission: 0.1 }),
      trade({ symbol: 'BTC-USDT-SWAP', action: 'close', side: 'sell', pos_side: 'long', value: 112, pnl: 12, commission: 0.1 }),
      trade({ symbol: 'ETH-USDT-SWAP', action: 'open', side: 'sell', pos_side: 'short', value: 50, commission: 0.05 }),
      trade({ symbol: 'ETH-USDT-SWAP', action: 'close', side: 'buy', pos_side: 'short', value: 55, pnl: -5, commission: 0.05 }),
    ], 1000)

    expect(rows).toHaveLength(2)
    expect(rows[0]).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      baseSymbol: 'BTC',
      closedTrades: 1,
      winningTrades: 1,
      losingTrades: 0,
      realizedPnl: 12,
      capitalContributionPct: 1.2,
      longPnl: 12,
      shortPnl: 0,
      commission: 0.2,
    })
    expect(rows[0].turnoverReturnPct).toBeCloseTo(10.7142857)
    expect(rows[1]).toMatchObject({
      symbol: 'ETH-USDT-SWAP',
      closedTrades: 1,
      winningTrades: 0,
      losingTrades: 1,
      realizedPnl: -5,
      capitalContributionPct: -0.5,
      longPnl: 0,
      shortPnl: -5,
      commission: 0.1,
    })
  })

  it('同一币种多次平仓会累计交易数和胜率', () => {
    const rows = buildBacktestSymbolPerformance([
      trade({ symbol: 'SOL-USDT-SWAP', action: 'close', side: 'sell', pos_side: 'long', value: 100, pnl: 8 }),
      trade({ symbol: 'SOL-USDT-SWAP', action: 'close', side: 'sell', pos_side: 'long', value: 100, pnl: -2 }),
      trade({ symbol: 'SOL-USDT-SWAP', action: 'close', side: 'buy', pos_side: 'short', value: 100, pnl: 4 }),
    ], 1000)

    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({
      closedTrades: 3,
      winningTrades: 2,
      losingTrades: 1,
      realizedPnl: 10,
      longPnl: 6,
      shortPnl: 4,
    })
    expect(rows[0].winRatePct).toBeCloseTo(66.6666667)
  })

  it('保留标的、动作和持仓方向的大小写与空格归一化', () => {
    const rows = buildBacktestSymbolPerformance([
      trade({ symbol: ' btc-usdt-swap ', action: ' CLOSE ', side: 'buy', pos_side: ' SHORT ', value: 100, pnl: 3 }),
      trade({ symbol: 'BTC-USDT-SWAP', action: 'close', side: 'sell', pos_side: 'long', value: 100, pnl: 7 }),
    ], 1000)

    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      baseSymbol: 'BTC',
      closedTrades: 2,
      realizedPnl: 10,
      longPnl: 7,
      shortPnl: 3,
    })
  })
})

function trade(overrides: Partial<BacktestTrade>): BacktestTrade {
  return {
    timestamp: 0,
    datetime: '',
    entry_time: '',
    exit_time: '',
    side: 'buy',
    action: '',
    pos_side: '',
    price: 0,
    entry_price: 0,
    exit_price: 0,
    quantity: 0,
    value: 0,
    commission: 0,
    pnl: 0,
    pnl_pct: 0,
    funding: 0,
    equity: 0,
    reason: '',
    ...overrides,
  }
}
