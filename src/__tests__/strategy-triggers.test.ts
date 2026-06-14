import { describe, expect, it } from 'vitest'
import type { BacktestTrade, LiveOrder } from '@/types'
import {
  backtestTradesToMarkers,
  liveOrdersToMarkers,
  toChartSeriesMarkers,
} from '@/utils/strategyTriggers'

describe('策略触发标记转换', () => {
  it('将回测开空和平仓事件转换为触发标记', () => {
    const trades: BacktestTrade[] = [
      trade({ timestamp: 60_000, side: 'sell', action: 'open', pos_side: 'short', price: 100, reason: 'overextension' }),
      trade({ timestamp: 120_000, side: 'buy', action: 'close', pos_side: 'short', price: 95, pnl: 12.3, reason: 'max_hold_bars' }),
    ]

    const markers = backtestTradesToMarkers(trades)

    expect(markers).toHaveLength(2)
    expect(markers[0]).toMatchObject({ kind: 'entry', label: '开空', side: 'sell', price: 100 })
    expect(markers[1]).toMatchObject({ kind: 'exit', label: '平空+12.30', side: 'buy', price: 95 })
  })

  it('忽略没有当前事件 action 的回测交易', () => {
    const markers = backtestTradesToMarkers([
      trade({
        entry_time: '2026-05-28T00:00:00.000Z',
        exit_time: '2026-05-28T01:00:00.000Z',
        side: 'buy',
        entry_price: 100,
        exit_price: 103,
        pnl: 3,
        pnl_pct: 3,
      }),
    ])

    expect(markers).toEqual([])
  })

  it('回测平仓事件缺少 pos_side 时按 close action 解释订单方向', () => {
    const markers = backtestTradesToMarkers([
      trade({
        timestamp: 120_000,
        side: 'sell',
        action: 'close',
        price: 110,
        pnl: 4,
        reason: 'take_profit',
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      kind: 'exit',
      label: '平多+4.00',
      side: 'sell',
      price: 110,
    })
  })

  it('实盘订单优先使用 timestamp 对齐触发时间', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 7,
        side: 'sell',
        status: 'filled',
        action: 'open_position',
        timestamp: 180_000,
        created_at: 240_000,
        price: 101,
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      timestamp: 180_000,
      kind: 'entry',
      label: '开空',
      source: 'simulated',
    })
  })

  it('实盘订单标记优先使用真实成交均价并显示成交摘要', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 9,
        side: 'buy',
        status: 'filled',
        action: 'open_position',
        timestamp: 180_000,
        price: 101,
        fill_count: 2,
        avg_fill_price: 102.5,
        filled_size: 1.25,
        total_fee: -0.03,
        fee_ccy: 'USDT',
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      price: 102.5,
      detail: 'open_position · filled · 成交 1.2500 @ 102.5000 fee -0.030000 USDT',
    })
  })

  it('实盘订单的无效 timestamp 会回退到创建时间', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 8,
        side: 'buy',
        status: 'filled',
        action: 'open_position',
        timestamp: Number.POSITIVE_INFINITY,
        created_at: 240_000,
        price: 101,
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      timestamp: 240_000,
      kind: 'entry',
      label: '开多',
    })
  })

  it('风控和阻塞订单显示为拦截类标记', () => {
    const markers = liveOrdersToMarkers([
      order({ id: 1, status: 'risk_blocked', timestamp: 60_000, price: 100 }),
      order({ id: 2, status: 'blocked', timestamp: 120_000, price: 101 }),
    ])

    expect(markers.map(marker => marker.kind)).toEqual(['risk', 'blocked'])
    expect(markers.map(marker => marker.label)).toEqual(['风控', '拦截'])
  })

  it('close_position 即使状态未 closed 也显示为退出标记', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 3,
        side: 'buy',
        status: 'filled',
        action: 'close_position',
        timestamp: 180_000,
        price: 99,
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      kind: 'exit',
      label: '平仓',
      side: 'buy',
      price: 99,
    })
  })

  it('交易所 close_position 提交记录显示为退出标记而不是反向开仓', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 4,
        side: 'sell',
        status: 'submitted',
        action: 'close_position',
        timestamp: 180_000,
        price: 99,
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      kind: 'exit',
      label: '平仓',
      side: 'sell',
      price: 99,
    })
  })

  it('保护单动作标记显示为保护单标签', () => {
    const markers = liveOrdersToMarkers([
      order({
        id: 5,
        side: 'sell',
        status: 'submit_failed',
        success: false,
        action: 'place_risk_order',
        timestamp: 180_000,
        price: 94,
      }),
    ])

    expect(markers).toHaveLength(1)
    expect(markers[0]).toMatchObject({
      kind: 'blocked',
      label: '拦截',
      side: 'sell',
      price: 94,
    })
  })

  it('图表 marker 会按周期对齐到 K 线 bucket 起点', () => {
    const chartMarkers = toChartSeriesMarkers([
      {
        id: 'm1',
        timestamp: 16 * 60_000,
        price: 100,
        side: 'buy',
        kind: 'entry',
        source: 'backtest',
        label: '开多',
      },
    ], '15m')

    expect(chartMarkers[0]).toMatchObject({
      time: 15 * 60,
      position: 'belowBar',
      shape: 'arrowUp',
    })
  })

  it('图表 marker 会忽略非有限时间戳', () => {
    const chartMarkers = toChartSeriesMarkers([
      {
        id: 'bad',
        timestamp: Number.POSITIVE_INFINITY,
        price: 100,
        side: 'buy',
        kind: 'entry',
        source: 'backtest',
        label: '开多',
      },
    ], '15m')

    expect(chartMarkers).toEqual([])
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
    quantity: 1,
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

function order(overrides: Partial<LiveOrder> & { price?: number }): LiveOrder {
  const { price, ...rest } = overrides
  return {
    id: 0,
    ord_id: '',
    client_order_id: '',
    parent_order_id: '',
    parent_client_order_id: '',
    actual_order_id: '',
    actual_client_order_id: '',
    inst_id: 'BTC-USDT-SWAP',
    symbol: 'BTC-USDT-SWAP',
    order_type: 'market',
    side: 'sell',
    sz: 1,
    px: price ?? rest.px ?? 100,
    fill_count: 0,
    filled_size: null,
    filled_quantity: null,
    avg_fill_price: null,
    fill_notional: null,
    remaining_size: null,
    total_fee: null,
    fee_ccy: null,
    first_fill_ts: null,
    last_fill_ts: null,
    fill_source: '',
    action: '',
    success: true,
    status: 'filled',
    error_message: '',
    mode: 'simulated',
    strategy_id: 'strategy',
    strategy_name: 'Strategy',
    run_id: 'run',
    timestamp: 0,
    arrival_ts: null,
    arrival_mid_px: null,
    arrival_bid_px: null,
    arrival_ask_px: null,
    created_at: 0,
    ...rest,
    reference_price: rest.reference_price ?? null,
    reference_price_source: rest.reference_price_source ?? '',
    reference_price_missing: rest.reference_price_missing ?? false,
  }
}
