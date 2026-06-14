import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  fetchAccount,
  fetchCostBasis,
  fetchFills,
  fetchLocalFills,
  fetchOrders,
  fetchPerformance,
  fetchPositions,
  fetchRiskControl,
  fetchSpotHoldings,
  fetchContractAccountConfig,
  fetchContractLeverage,
  normalizePrivateAccountEvent,
  placeOrder,
  setLeverage,
  setPositionMode,
  syncLocalFillsHistory,
} from '@/api/trading'

const invokeMock = vi.mocked(invoke)

describe('交易 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('账户、持仓、挂单、成交和现货持仓只消费 direct typed payload', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          total_eq: 1000,
          iso_eq: 900,
          adj_eq: 800,
          usdt_balance: 700,
          usdt_available: 680,
          usdt_equity_usd: 691,
          account: { total_eq: 1 },
          details: [{
            ccy: 'BTC',
            cash_bal: 1,
            avail_bal: 0.8,
            avail_eq: 0.8,
            frozen_bal: 0.2,
            ord_frozen: 0.2,
            eq: 1,
            eq_usd: 70000,
            dis_eq: 69000,
            u_time: 1779926400000,
          }],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            inst_id: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            pos_side: 'short',
            pos: 2,
            avg_px: 70000,
            upl: 12.5,
            upl_ratio: 0.01,
            lever: 3,
            margin: 100,
            mark_px: 69900,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            ord_id: 'order-1',
            inst_id: 'BTC-USDT-SWAP',
            side: 'sell',
            ord_type: 'market',
            sz: 2,
            px: 70000,
            state: 'live',
            c_time: 1779926400000,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            trade_id: 'fill-1',
            inst_id: 'BTC-USDT-SWAP',
            ord_id: 'order-1',
            side: 'buy',
            fill_px: 69900,
            fill_sz: 2,
            fee: 0.1,
            fee_ccy: 'USDT',
            ts: 1779926400000,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{ ccy: 'USDT', total: 1000 }],
      })

    await expect(fetchAccount('simulated')).resolves.toEqual({
      total_eq: 1000,
      iso_eq: 900,
      adj_eq: 800,
      usdt_balance: 700,
      usdt_available: 680,
      usdt_equity_usd: 691,
      details: [{
        ccy: 'BTC',
        total: 1,
        available: 0.8,
        frozen: 0.2,
        cash_bal: 1,
        avail_bal: 0.8,
        avail_eq: 0.8,
        eq: 1,
        eq_usd: 70000,
        dis_eq: 69000,
        ord_frozen: 0.2,
        u_time: 1779926400000,
      }],
    })
    await expect(fetchPositions('simulated')).resolves.toMatchObject([
      {
        inst_id: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        pos_side: 'short',
        pos: 2,
      },
    ])
    await expect(fetchOrders('simulated')).resolves.toMatchObject([
      {
        ord_id: 'order-1',
        inst_id: 'BTC-USDT-SWAP',
        side: 'sell',
        ord_type: 'market',
      },
    ])
    await expect(fetchFills(1, 'simulated')).resolves.toMatchObject([
      {
        fill_id: 'fill-1',
        inst_id: 'BTC-USDT-SWAP',
        ord_id: 'order-1',
        side: 'buy',
        fill_px: 69900,
      },
    ])
    await expect(fetchSpotHoldings('simulated')).resolves.toEqual([{ ccy: 'USDT', total: 1000 }])
  })

  it('账户和交易活动保留后端 null 经济字段，不伪造成 0', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          total_eq: null,
          iso_eq: null,
          adj_eq: null,
          usdt_balance: null,
          usdt_available: null,
          usdt_equity_usd: null,
          details: [{
            ccy: 'USDT',
            cash_bal: null,
            avail_bal: null,
            avail_eq: null,
            frozen_bal: null,
            ord_frozen: null,
            eq: null,
            eq_usd: null,
            dis_eq: null,
          }],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          pos_side: '',
          pos: null,
          avg_px: null,
          upl: null,
          upl_ratio: null,
          lever: null,
          margin: null,
          mark_px: null,
        }],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          ord_id: 'order-null',
          inst_id: 'BTC-USDT-SWAP',
          side: 'buy',
          ord_type: 'limit',
          sz: null,
          px: null,
          fill_sz: null,
          fill_px: null,
          avg_px: null,
          pnl: null,
          c_time: 1779926400000,
        }],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          trade_id: 'fill-null',
          inst_id: 'BTC-USDT-SWAP',
          ord_id: 'order-null',
          side: 'sell',
          fill_px: null,
          fill_sz: null,
          fee: null,
          fee_ccy: 'USDT',
          ts: 1779926400000,
        }],
      })

    await expect(fetchAccount('simulated')).resolves.toEqual({
      total_eq: null,
      iso_eq: null,
      adj_eq: null,
      usdt_balance: null,
      usdt_available: null,
      usdt_equity_usd: null,
      details: [],
    })
    await expect(fetchPositions('simulated')).resolves.toMatchObject([{
      pos_side: '',
      pos: null,
      avg_px: null,
      upl: null,
      upl_ratio: null,
      lever: null,
      margin: null,
      mark_px: null,
    }])
    await expect(fetchOrders('simulated')).resolves.toMatchObject([{
      ord_id: 'order-null',
      sz: null,
      px: null,
      fill_sz: null,
      fill_px: null,
      avg_px: null,
      pnl: null,
    }])
    await expect(fetchFills(1, 'simulated')).resolves.toMatchObject([{
      fill_id: 'fill-null',
      fill_px: null,
      fill_sz: null,
      fee: null,
    }])
  })

  it('成本、历史成交、绩效和风控配置只消费 direct typed payload', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            ccy: 'BTC',
            total_qty: 0.2,
            total_cost: 14000,
            avg_cost: 70000,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            id: '7',
            inst_id: 'ETH-USDT-SWAP',
            ccy: 'ETH',
            side: 'sell',
            fill_px: 3600,
            fill_sz: 0.5,
            fee: 0.2,
            ts: 1779926400000,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [
          {
            inst_id: 'ALL',
            total_trades: 3,
            win_rate: 0.67,
            total_pnl: 18.5,
            profit_factor: 1.8,
            largest_win: 12,
            largest_loss: -3,
          },
        ],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          enabled: false,
          max_single_loss_ratio: 0.05,
          max_position_pct: 0.25,
          max_order_value: 1000,
        },
      })

    await expect(fetchCostBasis('simulated')).resolves.toEqual([
      {
        ccy: 'BTC',
        total_quantity: 0.2,
        total_cost: 14000,
        avg_price: 70000,
        unrealized_pnl: 0,
      },
    ])

    await expect(fetchLocalFills('simulated')).resolves.toMatchObject([
      {
        id: '7',
        inst_id: 'ETH-USDT-SWAP',
        ccy: 'ETH',
        side: 'sell',
        quantity: 0.5,
        price: 3600,
        fee: 0.2,
        total_cost: 1800,
      },
    ])

    await expect(fetchPerformance('simulated')).resolves.toEqual([
      {
        inst_id: 'ALL',
        total_trades: 3,
        win_rate: 0.67,
        total_pnl: 18.5,
        profit_factor: 1.8,
        largest_win: 12,
        largest_loss: -3,
      },
    ])

    await expect(fetchRiskControl()).resolves.toMatchObject({
      enabled: false,
      max_single_loss_ratio: 0.05,
      max_position_pct: 0.25,
      max_order_value: 1000,
    })
  })

  it('本地成交价格或数量未知时不返回假 0 成交', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: 'bad-fill',
          inst_id: 'BTC-USDT-SWAP',
          ccy: 'BTC',
          side: 'buy',
          fill_px: null,
          fill_sz: null,
          fee: null,
          ts: 0,
        },
        {
          id: 'valid-fill',
          inst_id: 'ETH-USDT-SWAP',
          ccy: 'ETH',
          side: 'sell',
          fill_px: 3600,
          fill_sz: 0.5,
          fee: 0.2,
          ts: 0,
        },
      ],
    })

    await expect(fetchLocalFills('simulated')).resolves.toEqual([{
      id: 'valid-fill',
      inst_id: 'ETH-USDT-SWAP',
      ccy: 'ETH',
      side: 'sell',
      quantity: 0.5,
      price: 3600,
      fee: 0.2,
      total_cost: 1800,
      fill_time: '',
    }])
  })

  it('本地成交手续费未知或非法时不伪装成 0', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: 'missing-fee',
          inst_id: 'BTC-USDT-SWAP',
          ccy: 'BTC',
          side: 'buy',
          fill_px: 70000,
          fill_sz: 0.01,
          fee: null,
          ts: 1779926400000,
        },
        {
          id: 'bad-fee',
          inst_id: 'ETH-USDT-SWAP',
          ccy: 'ETH',
          side: 'sell',
          fill_px: 3600,
          fill_sz: 0.5,
          fee: 'bad-fee',
          ts: 1779926400000,
        },
      ],
    })

    await expect(fetchLocalFills('simulated')).resolves.toEqual([
      {
        id: 'missing-fee',
        inst_id: 'BTC-USDT-SWAP',
        ccy: 'BTC',
        side: 'buy',
        quantity: 0.01,
        price: 70000,
        fee: null,
        total_cost: 700,
        fill_time: '2026-05-28T00:00:00.000Z',
      },
      {
        id: 'bad-fee',
        inst_id: 'ETH-USDT-SWAP',
        ccy: 'ETH',
        side: 'sell',
        quantity: 0.5,
        price: 3600,
        fee: null,
        total_cost: 1800,
        fill_time: '2026-05-28T00:00:00.000Z',
      },
    ])
  })

  it('成本基准关键数值未知时不返回假 0 成本记录', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          ccy: 'BTC',
          total_qty: null,
          total_cost: null,
          avg_cost: null,
        },
        {
          ccy: 'ETH',
          total_qty: 0.5,
          total_cost: 1800,
          avg_cost: 3600,
        },
      ],
    })

    await expect(fetchCostBasis('simulated')).resolves.toEqual([{
      ccy: 'ETH',
      total_quantity: 0.5,
      total_cost: 1800,
      avg_price: 3600,
      unrealized_pnl: 0,
    }])
  })

  it('交易绩效收益指标未知或非法时不伪装成 0', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          inst_id: 'ALL',
          total_trades: 2,
          win_rate: null,
          total_pnl: null,
          profit_factor: null,
          largest_win: null,
          largest_loss: null,
        },
        {
          inst_id: 'BAD-NUMERIC',
          total_trades: 1,
          win_rate: '0.5',
          total_pnl: '12.3',
          profit_factor: '2.1',
          largest_win: '15',
          largest_loss: '-3',
        },
        {
          inst_id: 'BAD-COUNT',
          total_trades: '2',
          win_rate: 0.5,
          total_pnl: 12.3,
          profit_factor: 2.1,
          largest_win: 15,
          largest_loss: -3,
        },
      ],
    })

    await expect(fetchPerformance('simulated')).resolves.toEqual([
      {
        inst_id: 'ALL',
        total_trades: 2,
        win_rate: null,
        total_pnl: null,
        profit_factor: null,
        largest_win: null,
        largest_loss: null,
      },
      {
        inst_id: 'BAD-NUMERIC',
        total_trades: 1,
        win_rate: null,
        total_pnl: null,
        profit_factor: null,
        largest_win: null,
        largest_loss: null,
      },
    ])
  })

  it('同步 OKX 历史成交到 local_fills 使用显式 POST 契约', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        mode: 'simulated',
        inst_type: 'SWAP',
        inst_id: 'ETH-USDT-SWAP',
        fetched: 12,
        stored: 11,
        skipped_missing_trade_id: 1,
        arrival_matched: 3,
        note: 'synced',
      },
    })

    await expect(syncLocalFillsHistory({
      mode: 'simulated',
      inst_type: 'SWAP',
      inst_id: 'ETH-USDT-SWAP',
      limit: 50,
      after: '100',
      before: '200',
    })).resolves.toEqual({
      mode: 'simulated',
      inst_type: 'SWAP',
      inst_id: 'ETH-USDT-SWAP',
      fetched: 12,
      stored: 11,
      skipped_missing_trade_id: 1,
      arrival_matched: 3,
      note: 'synced',
    })

    expect(invokeMock.mock.calls[0][1]).toMatchObject({
      req: {
        method: 'POST',
        path: '/api/trading/local-fills/sync',
        body: {
          mode: 'simulated',
          inst_type: 'SWAP',
          inst_id: 'ETH-USDT-SWAP',
          limit: 50,
          after: '100',
          before: '200',
        },
      },
    })
  })

  it('不读取旧 wrapper，不解析字符串数字或方向别名', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: {
          total_eq: '1000',
          iso_eq: '900',
          adj_eq: '800',
          usdt_balance: '700',
          usdt_available: '680',
          usdt_equity_usd: '691',
          account: { total_eq: 1 },
          details: [{
            ccy: 'USDT',
            cash_bal: '700',
            avail_bal: '680',
            eq_usd: '691',
            raw: { cashBal: '700', eqUsd: '691' },
          }],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: {
          positions: [{
            inst_id: 'BTC-USDT-SWAP',
            pos: 2,
          }],
        },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          pos_side: 'SELL',
          pos: '2',
          avg_px: '70000',
          upl: '1',
          upl_ratio: '0.1',
          lever: '3',
          margin: '100',
          mark_px: '69900',
        }],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          ord_id: 'order-1',
          inst_id: 'BTC-USDT-SWAP',
          side: 'SHORT',
          ord_type: 'market',
          sz: '2',
          px: '70000',
          c_time: '1779926400000',
        }],
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          trade_id: 'fill-1',
          inst_id: 'BTC-USDT-SWAP',
          ord_id: 'order-1',
          side: 'SELL',
          fill_px: '69900',
          fill_sz: '2',
          fee: '0.1',
          ts: '1779926400000',
        }],
      })

    await expect(fetchAccount('simulated')).resolves.toEqual({
      total_eq: null,
      iso_eq: null,
      adj_eq: null,
      usdt_balance: null,
      usdt_available: null,
      usdt_equity_usd: null,
      details: [],
    })
    await expect(fetchPositions('simulated')).resolves.toEqual([])
    await expect(fetchPositions('simulated')).resolves.toMatchObject([{
      pos_side: '',
      pos: null,
      avg_px: null,
      lever: null,
    }])
    await expect(fetchOrders('simulated')).resolves.toMatchObject([{
      side: 'buy',
      sz: null,
      px: null,
      ctime: null,
    }])
    await expect(fetchFills(50, 'simulated')).resolves.toMatchObject([{
      side: 'sell',
      fill_px: null,
      fill_sz: null,
      fee: null,
      fill_time: null,
    }])
  })

  it('私有账户实时事件区分 USDT 余额和 USD 估值', () => {
    expect(normalizePrivateAccountEvent({
      mode: 'simulated',
      account: { total_eq: 88899.68, adj_eq: 88899.68 },
      data: {
        USDT: {
          ccy: 'USDT',
          availEq: '5000',
          availBal: '5000',
          cashBal: '5000',
          ordFrozen: '0',
          eq: '5000',
          eqUsd: '4991.8',
          disEq: '4991.7',
        },
      },
    })).toEqual({
      total_eq: 88899.68,
      iso_eq: null,
      adj_eq: 88899.68,
      usdt_balance: 5000,
      usdt_available: 5000,
      usdt_equity_usd: 4991.8,
      details: [{
        ccy: 'USDT',
        total: 5000,
        available: 5000,
        frozen: 0,
        cash_bal: 5000,
        avail_bal: 5000,
        avail_eq: 5000,
        eq: 5000,
        eq_usd: 4991.8,
        dis_eq: 4991.7,
        ord_frozen: 0,
        u_time: 0,
      }],
    })
  })

  it('私有账户实时事件缺少 isolated 和 adjusted equity 时保持未知', () => {
    expect(normalizePrivateAccountEvent({
      mode: 'simulated',
      account: { total_eq: 88899.68 },
      data: {},
    })).toEqual({
      total_eq: 88899.68,
      iso_eq: null,
      adj_eq: null,
      usdt_balance: null,
      usdt_available: null,
      usdt_equity_usd: null,
      details: [],
    })
  })

  it('合约配置和杠杆 API 透传交易模式、保证金模式和持仓方向', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: { posMode: 'long_short_mode' },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: [{
          instId: 'BTC-USDT-SWAP',
          mgnMode: 'cross',
          posSide: 'short',
          lever: '5',
        }],
      })
      .mockResolvedValueOnce({ code: 0, data: { ok: true } })
      .mockResolvedValueOnce({ code: 0, data: { ok: true } })

    await expect(fetchContractAccountConfig('simulated')).resolves.toEqual({
      pos_mode: 'long_short_mode',
      raw: { posMode: 'long_short_mode' },
    })
    await expect(fetchContractLeverage('BTC-USDT-SWAP', {
      mode: 'simulated',
      mgn_mode: 'cross',
    })).resolves.toEqual([{
      inst_id: 'BTC-USDT-SWAP',
      mgn_mode: 'cross',
      pos_side: 'short',
      lever: 5,
    }])
    await expect(setLeverage('BTC-USDT-SWAP', 5, {
      mode: 'simulated',
      mgn_mode: 'isolated',
      pos_side: 'short',
    })).resolves.toEqual({ ok: true })
    await expect(setPositionMode('net_mode', 'simulated')).resolves.toEqual({ ok: true })

    expect(invokeMock.mock.calls[0][1]).toMatchObject({
      req: {
        method: 'GET',
        path: '/api/trading/contract/account-config',
        params: { mode: 'simulated' },
      },
    })
    expect(invokeMock.mock.calls[1][1]).toMatchObject({
      req: {
        method: 'GET',
        path: '/api/trading/contract/leverage/BTC-USDT-SWAP',
        params: { mode: 'simulated', mgn_mode: 'cross' },
      },
    })
    expect(invokeMock.mock.calls[2][1]).toMatchObject({
      req: {
        method: 'POST',
        path: '/api/trading/contract/set-leverage',
        body: {
          inst_id: 'BTC-USDT-SWAP',
          lever: '5',
          mode: 'simulated',
          mgn_mode: 'isolated',
          pos_side: 'short',
        },
      },
    })
    expect(invokeMock.mock.calls[3][1]).toMatchObject({
      req: {
        method: 'POST',
        path: '/api/trading/contract/set-position-mode',
        body: { pos_mode: 'net_mode', mode: 'simulated' },
      },
    })
  })

  it('下单 API 清理空持仓方向，避免单向持仓误传 posSide', async () => {
    invokeMock.mockResolvedValueOnce({ code: 0, data: { ok: true } })

    await expect(placeOrder({
      inst_id: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      td_mode: 'cross',
      side: 'sell',
      ord_type: 'market',
      sz: 1,
      pos_side: undefined,
      reduce_only: undefined,
      mode: 'simulated',
    })).resolves.toEqual({ ok: true })

    expect(invokeMock.mock.calls[0][1]).toMatchObject({
      req: {
        method: 'POST',
        path: '/api/trading/order',
        body: {
          inst_id: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          td_mode: 'cross',
          side: 'sell',
          ord_type: 'market',
          sz: '1',
          px: '',
          mode: 'simulated',
        },
      },
    })
    expect((invokeMock.mock.calls[0][1] as any).req.body).not.toHaveProperty('pos_side')
    expect((invokeMock.mock.calls[0][1] as any).req.body).not.toHaveProperty('reduce_only')
  })
})
