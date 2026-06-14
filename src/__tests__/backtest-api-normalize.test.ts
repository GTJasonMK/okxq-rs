import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  fetchBacktestDetail,
  fetchBacktestHistory,
  fetchStrategies,
  runBacktestMonteCarloAnalysis,
} from '@/api/backtest'

const invokeMock = vi.mocked(invoke)

describe('回测 API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('策略元数据只接受运行契约字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: 'runtime_candidate_breakout_v1',
          name: 'Runtime Candidate Breakout V1',
          description: 'Self-contained runtime candidate',
          runtime: {
            symbol: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            timeframe: '15m',
            initial_capital: 1000,
            position_size: 0.15,
            stop_loss: 0.03,
            take_profit: 0.06,
            params: { leverage: 3 },
          },
          visualization: { indicator_series: [] },
          decision_contract: { reason_codes: ['candidate_breakout_long'] },
        },
      ],
    })

    const strategies = await fetchStrategies()

    expect(strategies).toEqual([
      expect.objectContaining({
        id: 'runtime_candidate_breakout_v1',
        name: 'Runtime Candidate Breakout V1',
        description: 'Self-contained runtime candidate',
        runtime: expect.objectContaining({
          symbol: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          initial_capital: 1000,
          position_size: 0.15,
          stop_loss: 0.03,
          take_profit: 0.06,
          params: { leverage: 3 },
        }),
        visualization: { indicator_series: [] },
        decision_contract: { reason_codes: ['candidate_breakout_long'] },
      }),
    ])
  })

  it('蒙特卡洛分析使用后端 block bootstrap 契约', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        result_id: 42,
        num_trades: 8,
        initial_capital: 1000,
        analysis: {
          num_simulations: 500,
          sampling_method: 'circular_block_bootstrap',
          block_size: 4,
          original_final_equity: 1040,
          original_max_drawdown: 3.2,
          equity_percentiles: [{ '5%': 960 }, { '95%': 1120 }],
          drawdown_percentiles: [{ '5%': 1.1 }, { '95%': 8.5 }],
          mean_final_equity: 1030,
          std_final_equity: 28,
          median_final_equity: 1032,
          prob_profit: 72,
          prob_original_beat: 44,
          worst_case_equity: 940,
          best_case_equity: 1150,
        },
      },
    })

    const result = await runBacktestMonteCarloAnalysis('42', {
      num_simulations: 500,
      block_size: 4,
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/backtest/monte-carlo/42',
        params: undefined,
        body: {
          num_simulations: 500,
          block_size: 4,
        },
      },
    })
    expect(result.analysis.sampling_method).toBe('circular_block_bootstrap')
    expect(result.analysis.block_size).toBe(4)
    expect(result.analysis.equity_percentiles[0]).toEqual({ '5%': 960 })
  })

  it('详情只从当前后端 snake_case payload 生成前端视图模型', async () => {
    const entryTime = '2026-05-28T00:00:00.000Z'
    const exitTime = '2026-05-28T01:00:00.000Z'
    const entryTs = Date.parse(entryTime)
    const exitTs = Date.parse(exitTime)

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 42,
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'ETH-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        days: 14,
        initial_capital: 1000,
        final_capital: 1030,
        total_return: 3,
        sharpe_ratio: 1.2,
        max_drawdown: 4.5,
        win_rate: 100,
        total_trades: 1,
        winning_trades: 1,
        losing_trades: 0,
        profit_factor: 3.4,
        params_json: JSON.stringify({
          leverage: 3,
          stop_loss: 0.02,
          strict_context_gating: false,
        }),
        backtest_result_integrity: {
          status: 'warning',
          issues: ['runtime_action_summary_warnings'],
        },
        runtime_execution_stamp: {
          schema: 'runtime_execution_stamp_v1',
          strategy: {
            project_relative_path: 'strategies/runtime/ml_trade_selector_forward_candidate_v1.py',
            sha256: 'strategy-sha',
          },
          runner: {
            project_relative_path: 'src-tauri/python/strategy_runner.py',
            sha256: 'runner-sha',
          },
        },
        trade_events_total: 2,
        trades_truncated: false,
        candles: [
          { timestamp: entryTs, open: 100, high: 105, low: 99, close: 104, volume: 12 },
        ],
        equity_curve: [
          { timestamp: entryTs, equity: 1000 },
          { timestamp: exitTs, equity: 1030 },
        ],
        trades: [
          {
            timestamp: entryTs,
            datetime: entryTime,
            side: 'sell',
            price: 100,
            quantity: 2,
            value: 200,
            commission: 0.2,
            pnl: null,
            reason: 'overextension_entry',
            metadata: {
              pos_side: 'short',
              action: 'open_position',
              funding: 0,
              equity: 999.8,
            },
          },
          {
            timestamp: exitTs,
            datetime: exitTime,
            side: 'buy',
            price: 95,
            quantity: 2,
            value: 190,
            commission: 0.2,
            pnl: 10,
            reason: 'max_hold_bars',
            metadata: {
              pos_side: 'short',
              action: 'close_position',
              funding: 0,
              equity: 1030,
            },
          },
        ],
        created_at: '2026-05-28T02:00:00.000Z',
      }),
    })

    const result = await fetchBacktestDetail('42')

    expect(result).toMatchObject({
      result_id: '42',
      strategy_id: 'multi_timeframe_dual_v12',
      strategy_name: 'V20',
      symbol: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      days: 14,
      initial_capital: 1000,
      final_equity: 1030,
      total_return_pct: 3,
      sharpe_ratio: 1.2,
      max_drawdown_pct: 4.5,
      win_rate_pct: 100,
      total_trades: 1,
      winning_trades: 1,
      losing_trades: 0,
      profit_factor: 3.4,
      params: {
        leverage: 3,
        stop_loss: 0.02,
        strict_context_gating: false,
      },
      backtest_result_integrity: {
        status: 'warning',
        issues: ['runtime_action_summary_warnings'],
      },
      runtime_execution_stamp: {
        schema: 'runtime_execution_stamp_v1',
        strategy: {
          project_relative_path: 'strategies/runtime/ml_trade_selector_forward_candidate_v1.py',
          sha256: 'strategy-sha',
        },
        runner: {
          project_relative_path: 'src-tauri/python/strategy_runner.py',
          sha256: 'runner-sha',
        },
      },
      trade_events_total: 2,
      trades_truncated: false,
      created_at: '2026-05-28T02:00:00.000Z',
    })
    expect(result.candles[0]).toMatchObject({
      inst_id: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      timestamp: entryTs,
      close: 104,
      volume: 12,
    })
    expect(result.equity_curve).toEqual([
      { time: entryTs, equity: 1000 },
      { time: exitTs, equity: 1030 },
    ])
    expect(result.trades[0]).toMatchObject({
      timestamp: entryTs,
      datetime: entryTime,
      entry_time: entryTime,
      exit_time: '',
      side: 'sell',
      action: 'open',
      pos_side: 'short',
      price: 100,
      entry_price: 100,
      exit_price: null,
      quantity: 2,
      value: 200,
      commission: 0.2,
      pnl: 0,
      funding: 0,
      equity: 999.8,
      reason: 'overextension_entry',
    })
    expect(result.trades[1]).toMatchObject({
      timestamp: exitTs,
      datetime: exitTime,
      entry_time: '',
      exit_time: exitTime,
      side: 'buy',
      action: 'close',
      pos_side: 'short',
      price: 95,
      entry_price: null,
      exit_price: 95,
      quantity: 2,
      value: 190,
      commission: 0.2,
      pnl: 10,
      funding: 0,
      equity: 1030,
      reason: 'max_hold_bars',
    })
  })

  it('详情交易兼容后端完整性检查支持的顶层 action 字段', async () => {
    const entryTime = '2026-05-28T00:00:00.000Z'
    const exitTime = '2026-05-28T01:00:00.000Z'
    const entryTs = Date.parse(entryTime)
    const exitTs = Date.parse(exitTime)

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'top-level-action',
        trades: [
          {
            timestamp: entryTs,
            datetime: entryTime,
            side: 'buy',
            action: 'open',
            price: 100,
            quantity: 1,
            value: 100,
            commission: 0.1,
            pnl: null,
            reason: 'entry',
            metadata: {
              pos_side: 'long',
              funding: 0,
              equity: 999.9,
            },
          },
          {
            timestamp: exitTs,
            datetime: exitTime,
            side: 'sell',
            action: 'close',
            price: 110,
            quantity: 1,
            value: 110,
            commission: 0.1,
            pnl: 10,
            reason: 'take_profit',
            metadata: {
              pos_side: 'long',
              funding: 0,
              equity: 1010,
            },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('top-level-action')

    expect(result.trades[0]).toMatchObject({
      action: 'open',
      entry_time: entryTime,
      exit_time: '',
      entry_price: 100,
      exit_price: null,
      pos_side: 'long',
    })
    expect(result.trades[1]).toMatchObject({
      action: 'close',
      entry_time: '',
      exit_time: exitTime,
      entry_price: null,
      exit_price: 110,
      pos_side: 'long',
      pnl: 10,
    })
  })

  it('详情交易缺少有效价格或方向时不伪造 0 价格和 buy 方向', async () => {
    const ts = Date.parse('2026-05-28T00:00:00.000Z')
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'malformed-trade-price-side',
        trades: [
          {
            timestamp: ts,
            datetime: '2026-05-28T00:00:00.000Z',
            side: 'short',
            action: 'close_position',
            price: 0,
            quantity: 2,
            value: 200,
            commission: 0.2,
            pnl: 10,
            reason: 'legacy_missing_price',
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('malformed-trade-price-side')
    const trade = result.trades[0]

    expect(trade.side).toBe('')
    expect(trade.action).toBe('close')
    expect(trade.price).toBeNaN()
    expect(trade.entry_price).toBeNull()
    expect(trade.exit_price).toBeNull()
  })

  it('详情交易保留 base 和 exchange 双数量口径', async () => {
    const entryTime = '2026-05-28T00:00:00.000Z'
    const entryTs = Date.parse(entryTime)
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'trade-quantity-units',
        trades: [
          {
            timestamp: entryTs,
            datetime: entryTime,
            side: 'sell',
            action: 'open_position',
            price: 0.103128,
            quantity: 3392,
            base_quantity: 3392,
            base_size: 3392,
            exchange_quantity: 339.2,
            size: 339.2,
            value: 349.810468,
            commission: 0.174905,
            pnl: null,
            metadata: { pos_side: 'short' },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('trade-quantity-units')

    expect(result.trades[0]).toMatchObject({
      quantity: 3392,
      base_quantity: 3392,
      exchange_quantity: 339.2,
      value: 349.810468,
    })
  })

  it('详情保留回测订单并从 fills 聚合成交状态', async () => {
    const submittedTs = Date.parse('2026-05-28T00:00:00.000Z')
    const fillTs = Date.parse('2026-05-28T00:01:00.000Z')
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'orders-result',
        orders: [
          {
            order_id: 'bt-1',
            client_order_id: 'bt-cl-1',
            inst_id: 'ARB-USDT-SWAP',
            symbol: 'ARB-USDT-SWAP',
            inst_type: 'SWAP',
            side: 'sell',
            pos_side: 'short',
            action: 'open_position',
            order_type: 'market',
            reference_price: 100,
            reference_price_source: 'entry_price_fallback',
            reference_price_missing: true,
            size: 339.2,
            filled_size: 339.2,
            remaining_size: 0,
            status: 'filled',
            success: true,
            action_timestamp: submittedTs,
            submitted_ts: submittedTs,
            updated_ts: fillTs,
          },
        ],
        fills: [
          {
            order_id: 'bt-1',
            client_order_id: 'bt-cl-1',
            symbol: 'ARB-USDT-SWAP',
            action: 'open_position',
            price: 0.103128,
            size: 339.2,
            value: 349.810468,
            commission: 0.174905,
            timestamp: fillTs,
          },
        ],
        rejected_orders: [
          {
            symbol: 'BTC-USDT-SWAP',
            inst_id: 'BTC-USDT-SWAP',
            action: 'modify_order',
            price: 0,
            avg_fill_price: 0,
            status: 'rejected',
            success: false,
            error_message: '订单不存在',
            timestamp: fillTs + 1,
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('orders-result')

    expect(result.fills).toHaveLength(1)
    expect(result.rejected_orders).toHaveLength(1)
    expect(result.orders).toHaveLength(2)
    expect(result.orders[0]).toMatchObject({
      ord_id: 'bt-1',
      client_order_id: 'bt-cl-1',
      inst_id: 'ARB-USDT-SWAP',
      side: 'sell',
      sz: 339.2,
      filled_size: 339.2,
      fill_count: 1,
      fill_notional: 349.810468,
      reference_price: 100,
      reference_price_source: 'entry_price_fallback',
      reference_price_missing: true,
      total_fee: 0.174905,
      first_fill_ts: fillTs,
      last_fill_ts: fillTs,
      action: 'open_position',
      success: true,
      status: 'filled',
      mode: 'simulated',
      run_id: 'orders-result',
      timestamp: fillTs,
    })
    expect(result.orders[0].avg_fill_price).toBeCloseTo(0.103128, 8)
    expect(result.orders[1]).toMatchObject({
      inst_id: 'BTC-USDT-SWAP',
      action: 'modify_order',
      px: null,
      avg_fill_price: null,
      success: false,
      status: 'rejected',
      error_message: '订单不存在',
    })
  })

  it('权益快照兼容历史撮合输出的 positions 数组字段', async () => {
    const ts = Date.parse('2026-05-28T00:00:00.000Z')
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'live-like-position-array',
        equity_curve: [
          {
            timestamp: ts,
            equity: 1008,
            cash: 990,
            position_notional: 400,
            unrealized_pnl: 18,
            position_side: 'long',
            positions: [
              {
                instId: 'ADA-USDT-SWAP',
                symbol: 'ADA-USDT-SWAP',
                instType: 'SWAP',
                posSide: 'long',
                pos: 143100,
                basePos: 1431,
                avgPx: 0.2447,
                markPx: 0.2471,
                mark_price_source: 'historical_last_close',
                mark_price_missing: false,
                notionalUsd: 353.6,
                upl: 3.4,
              },
            ],
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('live-like-position-array')
    const position = result.equity_snapshots?.[0].positions?.[0]

    expect(position).toMatchObject({
      symbol: 'ADA-USDT-SWAP',
      side: 'long',
      inst_type: 'SWAP',
      entry_price: 0.2447,
      mark_price: 0.2471,
      mark_price_source: 'historical_last_close',
      mark_price_missing: false,
      quantity: 1431,
      exchange_quantity: 143100,
      position_notional: 353.6,
      unrealized_pnl: 3.4,
    })
    expect(position?.entry_notional).toBeCloseTo(350.1657, 6)
  })

  it('历史列表直接消费后端数组并保留 summary 统计', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        backtestResult({
          id: 7,
          total_trades: 2,
          winning_trades: 1,
          losing_trades: 1,
        }),
      ],
    })

    const history = await fetchBacktestHistory()

    expect(history).toHaveLength(1)
    expect(history[0].result_id).toBe('7')
    expect(history[0].total_trades).toBe(2)
    expect(history[0].winning_trades).toBe(1)
    expect(history[0].losing_trades).toBe(1)
    expect(history[0].trades).toEqual([])
  })

  it('权益快照保留组合持仓状态', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'portfolio-result',
        equity_curve: [
          {
            timestamp: Date.parse('2026-05-28T00:00:00.000Z'),
            equity: 1000,
            cash: 400,
            position_value: 600,
            position_notional: 1200,
            unrealized_pnl: 15,
            position_side: 'portfolio',
            leverage: 2,
            positions: {
              'BTC-USDT-SWAP': {
                side: 'long',
                inst_type: 'SWAP',
                timeframe: '15m',
                entry_price: 100,
                quantity: 2,
                entry_timestamp: Date.parse('2026-05-28T00:00:00.000Z'),
                entry_notional: 200,
                entry_reason: 'open_long',
                mark_price: 112,
                position_notional: 224,
                unrealized_pnl: 24,
              },
              'ETH-USDT-SWAP': {
                symbol: 'ETH-USDT-SWAP',
                side: 'short',
                inst_type: 'SWAP',
                timeframe: '15m',
                entry_price: 2000,
                quantity: 0.1,
                entry_timestamp: Date.parse('2026-05-28T00:00:00.000Z'),
                entry_notional: 200,
                mark_price: 1900,
                notional: 190,
                unrealized_pnl: 10,
              },
            },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('portfolio-result')

    expect(result.equity_snapshots?.[0]).toMatchObject({
      equity: 1000,
      cash: 400,
      position_value: 600,
      position_notional: 1200,
      unrealized_pnl: 15,
      position_side: 'portfolio',
      leverage: 2,
    })
    expect(result.equity_snapshots?.[0].positions).toEqual([
      expect.objectContaining({
        symbol: 'BTC-USDT-SWAP',
        side: 'long',
        entry_price: 100,
        mark_price: 112,
        position_notional: 224,
        unrealized_pnl: 24,
      }),
      expect.objectContaining({
        symbol: 'ETH-USDT-SWAP',
        side: 'short',
        entry_price: 2000,
        mark_price: 1900,
        position_notional: 190,
        unrealized_pnl: 10,
      }),
    ])
  })

  it('权益快照不把缺失或无效的持仓经济字段改写为 0，并兼容数字字符串', async () => {
    const ts = Date.parse('2026-05-28T00:00:00.000Z')
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'sparse-position-result',
        equity_curve: [
          {
            timestamp: ts,
            equity: 1000,
            position_side: 'portfolio',
            position_value: null,
            positions: {
              'BTC-USDT-SWAP': {
                side: 'long',
                inst_type: 'SWAP',
                timeframe: '15m',
                entry_price: null,
                quantity: '1.25',
                entry_timestamp: '2026-05-28T00:00:00.000Z',
                entry_notional: null,
                stop_loss: null,
                take_profit: null,
                planned_exit_time: '1780000000000',
                planned_hold_bars: null,
                mark_price: null,
                position_notional: null,
                unrealized_pnl: null,
                unrealized_pnl_pct: '1.5',
              },
            },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('sparse-position-result')
    const snapshot = result.equity_snapshots?.[0]
    const position = snapshot?.positions?.[0]

    expect(snapshot).toMatchObject({
      cash: null,
      position_value: null,
      position_notional: null,
      unrealized_pnl: null,
      position_side: 'portfolio',
    })
    expect(position).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      side: 'long',
      entry_price: null,
      quantity: 1.25,
      entry_timestamp: null,
      entry_notional: null,
      stop_loss: null,
      take_profit: null,
      planned_exit_time: 1780000000000,
      planned_hold_bars: null,
      mark_price: null,
      position_notional: null,
      unrealized_pnl: null,
      unrealized_pnl_pct: 1.5,
    })
  })

  it('详情参数字段优先使用 params 对象并兼容 params_json', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: backtestResult({
          id: 'params-json',
          params_json: JSON.stringify({ leverage: 5, position_size: 0.2 }),
        }),
      })
      .mockResolvedValueOnce({
        code: 0,
        data: backtestResult({
          id: 'params-object',
          params: { leverage: 7 },
          params_json: JSON.stringify({ leverage: 5, position_size: 0.2 }),
        }),
      })
      .mockResolvedValueOnce({
        code: 0,
        data: backtestResult({
          id: 'bad-params-json',
          params_json: '{bad',
        }),
      })

    await expect(fetchBacktestDetail('params-json')).resolves.toMatchObject({
      result_id: 'params-json',
      params: { leverage: 5, position_size: 0.2 },
    })
    await expect(fetchBacktestDetail('params-object')).resolves.toMatchObject({
      result_id: 'params-object',
      params: { leverage: 7 },
    })
    await expect(fetchBacktestDetail('bad-params-json')).resolves.toMatchObject({
      result_id: 'bad-params-json',
      params: {},
    })
  })

  it('列表接口不读取旧 wrapper 字段', async () => {
    invokeMock
      .mockResolvedValueOnce({
        code: 0,
        data: { strategies: [{ id: 'wrapped_strategy', name: 'Wrapped' }] },
      })
      .mockResolvedValueOnce({
        code: 0,
        data: { results: [backtestResult({ id: 88 })] },
      })

    await expect(fetchStrategies()).resolves.toEqual([])
    await expect(fetchBacktestHistory()).resolves.toEqual([])
  })

  it('详情接口不读取旧 result wrapper', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: { result: backtestResult({ id: 88, total_trades: 9 }) },
    })

    const result = await fetchBacktestDetail('88')

    expect(result.result_id).toBe('')
    expect(result.total_trades).toBe(0)
    expect(result.trades).toEqual([])
  })

  it('详情带截断元数据时保留后端交易统计', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 9,
        total_trades: 500,
        winning_trades: 300,
        losing_trades: 200,
        trade_events_total: 1000,
        trades_truncated: true,
        trades: [
          {
            timestamp: Date.parse('2026-05-28T00:00:00.000Z'),
            datetime: '2026-05-28T00:00:00.000Z',
            side: 'sell',
            price: 100,
            quantity: 1,
            value: 100,
            commission: 0.1,
            pnl: null,
            reason: 'entry',
            metadata: { pos_side: 'short', action: 'open', funding: 0, equity: 999.9 },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('9')

    expect(result.trade_events_total).toBe(1000)
    expect(result.trades_truncated).toBe(true)
    expect(result.total_trades).toBe(500)
    expect(result.winning_trades).toBe(300)
    expect(result.losing_trades).toBe(200)
  })

  it('详情过滤无效图表 K 线和权益点', async () => {
    const validTs = Date.parse('2026-05-28T00:15:00.000Z')
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'chart-boundary',
        candles: [
          { timestamp: 0, open: 90, high: 91, low: 89, close: 90, volume: 1 },
          { timestamp: validTs, open: 100, high: 105, low: 99, close: 104, volume: 12 },
        ],
        equity_curve: [
          { timestamp: 0, equity: 0 },
          { timestamp: validTs, equity: 1030, cash: 100, position_value: 930 },
        ],
      }),
    })

    const result = await fetchBacktestDetail('chart-boundary')

    expect(result.candles).toHaveLength(1)
    expect(result.candles[0]).toMatchObject({ timestamp: validTs, close: 104 })
    expect(result.equity_curve).toEqual([{ time: validTs, equity: 1030 }])
    expect(result.equity_snapshots).toHaveLength(1)
    expect(result.equity_snapshots?.[0]).toMatchObject({
      time: validTs,
      equity: 1030,
      cash: 100,
      position_value: 930,
    })
  })

  it('详情兼容后端数字字符串，避免结果被归一化成全 0', async () => {
    const entryTime = '2026-05-28T00:00:00.000Z'
    const exitTime = '2026-05-28T01:00:00.000Z'
    const entryTs = Date.parse(entryTime)
    const exitTs = Date.parse(exitTime)
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'numeric-string-result',
        days: '14',
        initial_capital: '1000',
        final_capital: '1030',
        total_return: '3',
        sharpe_ratio: '1.2',
        max_drawdown: '4.5',
        win_rate: '100',
        total_trades: '1',
        winning_trades: '1',
        losing_trades: '0',
        profit_factor: '3.4',
        trade_events_total: '2',
        candles: [
          { timestamp: `${entryTs}`, open: '100', high: '105', low: '99', close: '104', volume: '12' },
        ],
        equity_curve: [
          {
            timestamp: `${entryTs}`,
            equity: '1000',
            cash: '900',
            position_notional: '200',
            unrealized_pnl: '8',
            position_side: 'portfolio',
            positions: {
              'BTC-USDT-SWAP': {
                side: 'long',
                inst_type: 'SWAP',
                timeframe: '15m',
                entry_price: '100',
                quantity: '2',
                mark_price: '104',
                position_notional: '208',
                unrealized_pnl: '8',
              },
            },
          },
        ],
        trades: [
          {
            timestamp: `${entryTs}`,
            datetime: entryTime,
            side: 'sell',
            price: '100',
            quantity: '2',
            value: '200',
            commission: '0.2',
            pnl: null,
            reason: 'entry',
            metadata: { pos_side: 'short', action: 'open_position', funding: '0', equity: '999.8' },
          },
          {
            timestamp: `${exitTs}`,
            datetime: exitTime,
            side: 'buy',
            price: '95',
            quantity: '2',
            value: '190',
            commission: '0.2',
            pnl: '10',
            reason: 'exit',
            metadata: { pos_side: 'short', action: 'close_position', funding: '0.1', equity: '1030' },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('numeric-string-result')

    expect(result).toMatchObject({
      days: 14,
      initial_capital: 1000,
      final_equity: 1030,
      total_return_pct: 3,
      sharpe_ratio: 1.2,
      max_drawdown_pct: 4.5,
      win_rate_pct: 100,
      total_trades: 1,
      winning_trades: 1,
      losing_trades: 0,
      profit_factor: 3.4,
      trade_events_total: 2,
    })
    expect(result.candles).toHaveLength(1)
    expect(result.candles[0]).toMatchObject({ timestamp: entryTs, close: 104, volume: 12 })
    expect(result.equity_snapshots?.[0]).toMatchObject({
      time: entryTs,
      equity: 1000,
      cash: 900,
      position_notional: 200,
      unrealized_pnl: 8,
      position_side: 'portfolio',
    })
    expect(result.equity_snapshots?.[0].positions?.[0]).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      entry_price: 100,
      quantity: 2,
      mark_price: 104,
      position_notional: 208,
      unrealized_pnl: 8,
    })
    expect(result.trades[0]).toMatchObject({
      timestamp: entryTs,
      entry_price: 100,
      exit_price: null,
      quantity: 2,
      value: 200,
      commission: 0.2,
      equity: 999.8,
    })
    expect(result.trades[1]).toMatchObject({
      timestamp: exitTs,
      entry_price: null,
      exit_price: 95,
      quantity: 2,
      value: 190,
      commission: 0.2,
      pnl: 10,
      funding: 0.1,
      equity: 1030,
    })
  })

  it('详情保留历史资金费事件并识别 funding 交易动作', async () => {
    const fundingTime = '2026-05-28T08:00:00.000Z'
    const fundingTs = Date.parse(fundingTime)
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'funding-result',
        funding_events: [
          {
            timestamp: fundingTs,
            symbol: 'BTC-USDT-SWAP',
            action: 'funding',
            funding: -0.12,
            funding_rate: 0.0003,
          },
        ],
        trades: [
          {
            timestamp: fundingTs,
            datetime: fundingTime,
            side: 'funding',
            pos_side: 'long',
            action: 'funding',
            price: 100,
            quantity: 2,
            value: 200,
            commission: 0,
            pnl: null,
            funding: '-0.12',
            reason: 'funding_rate:0.00030000',
            metadata: {
              pos_side: 'long',
              action: 'funding',
              symbol: 'BTC-USDT-SWAP',
              funding: '-0.12',
            },
          },
        ],
      }),
    })

    const result = await fetchBacktestDetail('funding-result')

    expect(result.funding_events ?? []).toHaveLength(1)
    expect(result.funding_events?.[0]).toMatchObject({
      symbol: 'BTC-USDT-SWAP',
      action: 'funding',
      funding: -0.12,
    })
    expect(result.trades[0]).toMatchObject({
      timestamp: fundingTs,
      action: 'funding',
      side: 'funding',
      pos_side: 'long',
      price: 100,
      quantity: 2,
      value: 200,
      funding: -0.12,
      pnl: 0,
    })
  })

  it('详情拒绝 ISO 时间戳字符串和方向别名，但识别后端动作名称', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: backtestResult({
        id: 'strict-id',
        days: '14',
        initial_capital: '1000',
        final_capital: '1030',
        total_return: '3',
        sharpe_ratio: '1.2',
        max_drawdown: '4.5',
        win_rate: '100',
        total_trades: '1',
        winning_trades: '1',
        losing_trades: '0',
        profit_factor: '3.4',
        trade_events_total: '2',
        trades_truncated: 'true',
        candles: [
          { timestamp: '2026-05-28T00:00:00.000Z', open: '100', high: '105', low: '99', close: '104', volume: '12' },
        ],
        equity_curve: [
          { timestamp: '2026-05-28T00:00:00.000Z', equity: '1000' },
        ],
        trades: [
          {
            timestamp: '2026-05-28T00:00:00.000Z',
            side: 'short',
            price: '100',
            quantity: '2',
            value: '200',
            commission: '0.2',
            pnl: '10',
            reason: 123,
            metadata: {
              pos_side: 'sell',
              action: 'open_position',
              funding: '0.1',
              equity: '1030',
            },
          },
        ],
        created_at: 123,
      }),
    })

    const result = await fetchBacktestDetail('strict-id')

    expect(result).toMatchObject({
      result_id: 'strict-id',
      days: 14,
      initial_capital: 1000,
      final_equity: 1030,
      total_return_pct: 3,
      sharpe_ratio: 1.2,
      max_drawdown_pct: 4.5,
      win_rate_pct: 100,
      total_trades: 1,
      winning_trades: 1,
      losing_trades: 0,
      profit_factor: 3.4,
      trade_events_total: 2,
      trades_truncated: false,
      created_at: '',
    })
    expect(result.candles).toEqual([])
    expect(result.equity_curve).toEqual([])
    expect(result.equity_snapshots).toEqual([])
    expect(result.trades[0]).toMatchObject({
      timestamp: 0,
      datetime: '',
      side: '',
      action: 'open',
      pos_side: '',
      price: 100,
      entry_price: 100,
      exit_price: null,
      quantity: 2,
      value: 200,
      commission: 0.2,
      pnl: 10,
      funding: 0.1,
      equity: 1030,
      reason: '',
    })
  })
})

function backtestResult(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    days: 30,
    initial_capital: 1000,
    final_capital: 1005,
    total_return: 0.5,
    sharpe_ratio: 0,
    max_drawdown: 0,
    win_rate: 100,
    total_trades: 0,
    winning_trades: 0,
    losing_trades: 0,
    profit_factor: 0,
    trade_events_total: 0,
    trades_truncated: false,
    orders: [],
    fills: [],
    rejected_orders: [],
    candles: [],
    indicators: {},
    equity_curve: [],
    trades: [],
    created_at: '2026-05-28T00:00:00.000Z',
    ...overrides,
  }
}
