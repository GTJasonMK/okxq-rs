import { afterEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import {
  fetchAvailableStrategies,
  fetchLiveEquity,
  fetchLiveExecutionLogs,
  fetchLiveExecutionPlans,
  fetchLiveOrders,
  fetchDecisionDiagnostics,
  fetchLiveStatus,
  startLiveStrategy,
} from '@/api/live'
import { tradingMode } from '@/api/live/shared'

const invokeMock = vi.mocked(invoke)

describe('模拟盘 live API snake_case 契约', () => {
  afterEach(() => {
    invokeMock.mockClear()
    invokeMock.mockResolvedValue({ code: 0, data: null })
  })

  it('订单列表只消费后端 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: 7,
          order_id: 'okx-order-123456789',
          client_order_id: 'client-order-1',
          parent_order_id: 'parent-order-1',
          parent_client_order_id: 'parent-client-order-1',
          inst_id: 'ETH-USDT-SWAP',
          symbol: 'ETH-USDT-SWAP',
          order_type: 'market',
          side: 'sell',
          size: 2.5,
          price: 3012.4,
          status: 'filled',
          action: 'open_position',
          timestamp: 1779926400000,
          arrival_ts: 1779926400123,
          success: true,
          fill_count: 2,
          filled_size: 2,
          filled_quantity: 2,
          avg_fill_price: 3012.45,
          fill_notional: 6024.9,
          remaining_size: 0.5,
          total_fee: -0.12,
          fee_ccy: 'USDT',
          first_fill_ts: 1779926400100,
          last_fill_ts: 1779926400200,
          fill_source: 'okx_private_ws',
          arrival_mid_px: 3012.3,
          arrival_bid_px: 3012.2,
          arrival_ask_px: 3012.5,
          created_at: '2026-05-28T00:01:00.000Z',
          strategy_id: 'multi_timeframe_dual_v12',
          strategy_name: 'V20',
          run_id: 'run-live',
          mode: 'live',
        },
      ],
    })

    const orders = await fetchLiveOrders({ limit: 1, mode: 'live', run_id: 'run-live' })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/live/orders',
        params: { limit: 1, mode: 'live', run_id: 'run-live' },
        body: undefined,
      },
    })
    expect(orders[0]).toMatchObject({
      id: 7,
      ord_id: 'okx-order-123456789',
      client_order_id: 'client-order-1',
      parent_order_id: 'parent-order-1',
      parent_client_order_id: 'parent-client-order-1',
      inst_id: 'ETH-USDT-SWAP',
      symbol: 'ETH-USDT-SWAP',
      order_type: 'market',
      side: 'sell',
      sz: 2.5,
      px: 3012.4,
      fill_count: 2,
      filled_size: 2,
      filled_quantity: 2,
      avg_fill_price: 3012.45,
      fill_notional: 6024.9,
      remaining_size: 0.5,
      total_fee: -0.12,
      fee_ccy: 'USDT',
      first_fill_ts: 1779926400100,
      last_fill_ts: 1779926400200,
      fill_source: 'okx_private_ws',
      status: 'filled',
      action: 'open_position',
      success: true,
      arrival_ts: 1779926400123,
      arrival_mid_px: 3012.3,
      arrival_bid_px: 3012.2,
      arrival_ask_px: 3012.5,
      strategy_id: 'multi_timeframe_dual_v12',
      strategy_name: 'V20',
      run_id: 'run-live',
      mode: 'live',
    })
    expect(orders[0].timestamp).toBe(Date.parse('2026-05-28T00:00:00.000Z'))
    expect(orders[0].created_at).toBe(Date.parse('2026-05-28T00:01:00.000Z'))
  })

  it('执行日志使用结构化阶段字段并保留 details', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          seq: 3,
          run_id: 'run-live',
          mode: 'live',
          strategy_id: 'strategy-live',
          strategy_name: 'Live Strategy',
          symbol: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          timestamp_ms: 1779926400123,
          time: '2026-05-28T00:00:00.123Z',
          stage: 'submit',
          level: 'success',
          message: 'OKX 订单已提交',
          details: { symbol: 'BTC-USDT-SWAP', size: 2 },
        },
      ],
    })

    const logs = await fetchLiveExecutionLogs({ mode: 'live', run_id: 'run-live', limit: 20 })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/live/execution-logs',
        params: { mode: 'live', run_id: 'run-live', limit: 20 },
        body: undefined,
      },
    })
    expect(logs).toEqual([
      {
        seq: 3,
        run_id: 'run-live',
        mode: 'live',
        strategy_id: 'strategy-live',
        strategy_name: 'Live Strategy',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        timestamp_ms: 1779926400123,
        time: '2026-05-28T00:00:00.123Z',
        stage: 'submit',
        level: 'success',
        message: 'OKX 订单已提交',
        details: { symbol: 'BTC-USDT-SWAP', size: 2 },
      },
    ])
  })

  it('退出计划列表只消费后端 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: 9,
          plan_key: 'plan-1',
          strategy_id: 'strategy-a',
          strategy_name: 'Strategy A',
          mode: 'live',
          entry_run_id: 'run-entry',
          exit_run_id: 'run-exit',
          symbol: 'ETH-USDT-SWAP',
          inst_id: 'ETH-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          entry_order_id: 'entry-order',
          entry_client_order_id: 'entry-client-order',
          entry_timestamp: 1779926400000,
          entry_side: 'buy',
          entry_price: 3012.4,
          close_side: 'sell',
          planned_exit_time: 1779927300000,
          planned_exit_reason: 'hold_bars_elapsed',
          planned_exit_contract: 'planned_exit_time_v1',
          status: 'scheduled',
          exit_order_id: 'exit-order',
          exit_client_order_id: 'exit-client-order',
          attempt_count: 2,
          next_attempt_at: 1779927360000,
          last_error: 'order canceled',
          created_at: '2026-05-28T00:00:00.000Z',
          updated_at: '2026-05-28T00:01:00.000Z',
        },
      ],
    })

    const plans = await fetchLiveExecutionPlans({ limit: 1, mode: 'live', run_id: 'run-entry' })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/live/execution-plans',
        params: { limit: 1, mode: 'live', run_id: 'run-entry' },
        body: undefined,
      },
    })
    expect(plans[0]).toMatchObject({
      id: 9,
      plan_key: 'plan-1',
      strategy_id: 'strategy-a',
      strategy_name: 'Strategy A',
      mode: 'live',
      entry_run_id: 'run-entry',
      exit_run_id: 'run-exit',
      symbol: 'ETH-USDT-SWAP',
      inst_id: 'ETH-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      entry_order_id: 'entry-order',
      entry_client_order_id: 'entry-client-order',
      entry_timestamp: 1779926400000,
      entry_side: 'buy',
      entry_price: 3012.4,
      close_side: 'sell',
      planned_exit_time: 1779927300000,
      planned_exit_reason: 'hold_bars_elapsed',
      planned_exit_contract: 'planned_exit_time_v1',
      status: 'scheduled',
      exit_order_id: 'exit-order',
      exit_client_order_id: 'exit-client-order',
      attempt_count: 2,
      next_attempt_at: 1779927360000,
      last_error: 'order canceled',
      created_at: Date.parse('2026-05-28T00:00:00.000Z'),
      updated_at: Date.parse('2026-05-28T00:01:00.000Z'),
    })
  })

  it('退出计划不解析字符串数字或字符串时间戳', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [{
        id: '9',
        entry_timestamp: '1779926400000',
        entry_price: '3012.4',
        planned_exit_time: '1779927300000',
        attempt_count: '2',
        next_attempt_at: '1779927360000',
        created_at: 1779926400000,
        updated_at: 1779926460000,
      }],
    })

    const plans = await fetchLiveExecutionPlans()

    expect(plans[0]).toMatchObject({
      id: 0,
      entry_timestamp: null,
      entry_price: null,
      planned_exit_time: null,
      attempt_count: 0,
      next_attempt_at: null,
      created_at: 0,
      updated_at: 0,
    })
  })

  it('策略和订单列表不再读取旧 wrapper 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [{
        id: 'runtime_candidate_breakout_v1',
        name: 'Runtime Candidate Breakout V1',
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
      }],
    })
    await expect(fetchAvailableStrategies()).resolves.toEqual([
      expect.objectContaining({
        id: 'runtime_candidate_breakout_v1',
        name: 'Runtime Candidate Breakout V1',
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
    expect(invokeMock).toHaveBeenLastCalledWith('local_api_request', {
      req: {
        method: 'GET',
        path: '/api/backtest/strategies',
        params: undefined,
        body: undefined,
      },
    })

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: { strategies: [{ id: 'legacy_strategy', name: 'Legacy' }] },
    })
    await expect(fetchAvailableStrategies()).resolves.toEqual([])

    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: { orders: [{ id: 1, side: 'sell' }] },
    })
    await expect(fetchLiveOrders()).resolves.toEqual([])
  })

  it('订单字段不解析字符串数字、字符串布尔或方向别名，缺失经济字段保持未知', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: [
        {
          id: '7',
          side: 'short',
          size: '2.5',
          quantity: 2.5,
          price: '3012.4',
          success: 'true',
          timestamp: '1779926400000',
          arrival_ts: '1779926400123',
          created_at: 1779926460000,
        },
      ],
    })

    const orders = await fetchLiveOrders()

    expect(orders[0]).toMatchObject({
      id: 0,
      side: '',
      sz: null,
      px: null,
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
      success: false,
      timestamp: null,
      arrival_ts: null,
      created_at: 0,
    })
  })

  it('响应运行模式归一拒绝旧模式别名', () => {
    expect(tradingMode(undefined)).toBe('simulated')
    expect(tradingMode('')).toBe('simulated')
    expect(tradingMode('live')).toBe('live')
    expect(tradingMode('simulated')).toBe('simulated')

    for (const mode of ['paper', 'demo', 'simulation', 'sandbox']) {
      expect(() => tradingMode(mode)).toThrow('运行模式只支持 live 或 simulated')
    }
  })

  it('状态只消费后端 snake_case 字段并从 status 推导 running', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        status: 'running',
        run_id: 'run-status',
        mode: 'live',
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        timeframe: '15m',
        inst_type: 'SWAP',
        risk_timeframe: '1m',
        last_action_time: '2026-05-28T00:00:00.000Z',
        last_action: 'open_position',
        total_actions: 12,
        total_orders: 9,
        successful_orders: 8,
        failed_orders: 1,
        error_message: '风控拦截 1 次',
        check_interval: 30,
        execution_mode: 'exchange_demo',
        last_price: 68123.4,
        last_action_strength: 0.87,
        last_action_reason: 'overextension',
        last_order_candle_ts: 1_780_000_000_000,
      },
    })

    const status = await fetchLiveStatus()

    expect(status).toMatchObject({
      status: 'running',
      running: true,
      run_id: 'run-status',
      mode: 'live',
      strategy_id: 'multi_timeframe_dual_v12',
      strategy_name: 'V20',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      last_action_time: '2026-05-28T00:00:00.000Z',
      last_action: 'open_position',
      actions_generated: 12,
      orders_placed: 9,
      successful_orders: 8,
      failed_orders: 1,
      error_message: '风控拦截 1 次',
      check_interval: 30,
      last_price: 68123.4,
      last_action_strength: 0.87,
      last_action_reason: 'overextension',
      last_order_candle_ts: 1_780_000_000_000,
    })
  })

  it('状态观测数值不把字符串字段归一化为 0', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        status: 'running',
        run_id: 'run-dirty-status',
        total_actions: '12',
        total_orders: '9',
        successful_orders: '8',
        failed_orders: '1',
        last_price: '68123.4',
        last_action_strength: '0.87',
        last_order_candle_ts: '1780000000000',
      },
    })

    const status = await fetchLiveStatus()

    expect(status).toMatchObject({
      status: 'running',
      running: true,
      actions_generated: null,
      orders_placed: null,
      successful_orders: null,
      failed_orders: null,
      last_price: null,
      last_action_strength: null,
      last_order_candle_ts: null,
    })
  })

  it('stopped/error 状态不会被误判为运行中', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        status: 'stopped',
        run_id: 'run-stopped',
      },
    })

    const status = await fetchLiveStatus()

    expect(status.status).toBe('stopped')
    expect(status.running).toBe(false)
  })

  it('权益快照和日汇总只消费 snake_case 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        run_id: 'run-equity',
        mode: 'simulated',
        count: 1,
        snapshots: [
          {
            id: 11,
            run_id: 'run-equity',
            strategy_id: 'multi_timeframe_dual_v12',
            strategy_name: 'V20',
            symbol: 'DOGE-USDT-SWAP',
            inst_id: 'DOGE-USDT-SWAP',
            timeframe: '5m',
            inst_type: 'SWAP',
            timestamp: 1779927300000,
            time: '2026-05-28 08:15:00',
            trading_day: '2026-05-28',
            price: 0.221,
            position_side: 'short',
            entry_price: 0.225,
            quantity: 1000,
            initial_capital: 1000,
            day_start_equity: 1005,
            equity: 1012.5,
            realized_pnl: 5,
            unrealized_pnl: 7.5,
            total_pnl: 12.5,
            total_pnl_pct: 1.25,
            today_pnl: 7.5,
            today_pnl_pct: 0.75,
            created_at: '2026-05-28T00:16:00.000Z',
          },
        ],
        daily: [
          {
            trading_day: '2026-05-28',
            start_timestamp: 1779926400000,
            end_timestamp: 1780012740000,
            start_time: '2026-05-28 08:00:00',
            end_time: '2026-05-29 07:59:00',
            snapshot_count: 3,
            first_equity: 1005,
            last_equity: 1012.5,
            day_start_equity: 1005,
            today_pnl: 7.5,
            today_pnl_pct: 0.75,
            total_pnl: 12.5,
            total_pnl_pct: 1.25,
            realized_pnl: 5,
            unrealized_pnl: 7.5,
          },
        ],
      },
    })

    const history = await fetchLiveEquity({ limit: 1, mode: 'simulated' })

    expect(history.run_id).toBe('run-equity')
    expect(history.count).toBe(1)
    expect(history.snapshots[0]).toMatchObject({
      id: 11,
      run_id: 'run-equity',
      strategy_id: 'multi_timeframe_dual_v12',
      strategy_name: 'V20',
      symbol: 'DOGE-USDT-SWAP',
      inst_id: 'DOGE-USDT-SWAP',
      timeframe: '5m',
      inst_type: 'SWAP',
      trading_day: '2026-05-28',
      price: 0.221,
      position_side: 'short',
      entry_price: 0.225,
      quantity: 1000,
      equity: 1012.5,
      total_pnl: 12.5,
      today_pnl: 7.5,
    })
    expect(history.snapshots[0].timestamp).toBe(Date.parse('2026-05-28T00:15:00.000Z'))
    expect(history.snapshots[0].created_at).toBe(Date.parse('2026-05-28T00:16:00.000Z'))
    expect(history.daily[0]).toMatchObject({
      trading_day: '2026-05-28',
      snapshot_count: 3,
      first_equity: 1005,
      last_equity: 1012.5,
      day_start_equity: 1005,
      today_pnl: 7.5,
      total_pnl: 12.5,
      realized_pnl: 5,
      unrealized_pnl: 7.5,
    })
    expect(history.daily[0].start_timestamp).toBe(Date.parse('2026-05-28T00:00:00.000Z'))
    expect(history.daily[0].end_timestamp).toBe(Date.parse('2026-05-28T23:59:00.000Z'))
  })

  it('权益快照和日汇总不把字符串经济字段归一化为 0', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        run_id: 'run-equity-dirty',
        mode: 'simulated',
        count: 1,
        snapshots: [
          {
            id: 11,
            run_id: 'run-equity-dirty',
            strategy_id: 'multi_timeframe_dual_v12',
            strategy_name: 'V20',
            symbol: 'DOGE-USDT-SWAP',
            inst_id: 'DOGE-USDT-SWAP',
            timeframe: '5m',
            inst_type: 'SWAP',
            timestamp: 1779927300000,
            time: '2026-05-28 08:15:00',
            trading_day: '2026-05-28',
            price: '0.221',
            position_side: 'short',
            entry_price: '0.225',
            quantity: '1000',
            initial_capital: '1000',
            day_start_equity: '1005',
            equity: '1012.5',
            realized_pnl: '5',
            unrealized_pnl: '7.5',
            total_pnl: '12.5',
            total_pnl_pct: '1.25',
            today_pnl: '7.5',
            today_pnl_pct: '0.75',
            created_at: '2026-05-28T00:16:00.000Z',
          },
        ],
        daily: [
          {
            trading_day: '2026-05-28',
            start_timestamp: 1779926400000,
            end_timestamp: 1780012740000,
            snapshot_count: '3',
            first_equity: '1005',
            last_equity: '1012.5',
            day_start_equity: '1005',
            today_pnl: '7.5',
            today_pnl_pct: '0.75',
            total_pnl: '12.5',
            total_pnl_pct: '1.25',
            realized_pnl: '5',
            unrealized_pnl: '7.5',
          },
        ],
      },
    })

    const history = await fetchLiveEquity({ limit: 1, mode: 'simulated' })

    expect(history.snapshots).toEqual([])
    expect(history.daily).toEqual([])
  })

  it('OKX balance 权益快照保留空的策略收益和持仓字段为未知', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        run_id: '',
        mode: 'simulated',
        count: 1,
        source: 'okx_account_balance',
        pnl_available: false,
        snapshots: [
          {
            id: 0,
            run_id: '',
            strategy_id: '',
            strategy_name: '',
            symbol: '',
            inst_id: '',
            timeframe: '1H',
            inst_type: 'SPOT',
            timestamp: 1779927300000,
            time: '2026-05-28T00:15:00.000Z',
            trading_day: '2026-05-28',
            price: null,
            position_side: null,
            entry_price: null,
            quantity: null,
            initial_capital: 1234.56,
            day_start_equity: 1234.56,
            equity: 1234.56,
            realized_pnl: null,
            unrealized_pnl: 10,
            total_pnl: null,
            total_pnl_pct: null,
            today_pnl: null,
            today_pnl_pct: null,
            created_at: '2026-05-28T00:16:00.000Z',
            pnl_available: false,
            source: 'okx_account_balance',
          },
        ],
        daily: [
          {
            trading_day: '2026-05-28',
            start_timestamp: 1779927300000,
            end_timestamp: 1779927300000,
            start_time: '2026-05-28T00:15:00.000Z',
            end_time: '2026-05-28T00:15:00.000Z',
            snapshot_count: 1,
            first_equity: 1234.56,
            last_equity: 1234.56,
            day_start_equity: 1234.56,
            today_pnl: null,
            today_pnl_pct: null,
            total_pnl: null,
            total_pnl_pct: null,
            realized_pnl: null,
            unrealized_pnl: 10,
            pnl_available: false,
          },
        ],
      },
    })

    const history = await fetchLiveEquity({ limit: 1, mode: 'simulated' })

    expect(history.source).toBe('okx_account_balance')
    expect(history.pnl_available).toBe(false)
    expect(history.snapshots).toHaveLength(1)
    expect(history.snapshots[0]).toMatchObject({
      equity: 1234.56,
      price: null,
      position_side: '',
      entry_price: null,
      quantity: null,
      realized_pnl: null,
      unrealized_pnl: 10,
      total_pnl: null,
      today_pnl: null,
      pnl_available: false,
    })
    expect(history.daily).toHaveLength(1)
    expect(history.daily[0]).toMatchObject({
      last_equity: 1234.56,
      today_pnl: null,
      total_pnl: null,
      realized_pnl: null,
      unrealized_pnl: 10,
      pnl_available: false,
    })
  })

  it('启动策略时保留用户选择的运行参数和策略参数', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        status: 'running',
        run_id: 'run-v20',
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        timeframe: '15m',
        inst_type: 'SWAP',
        mode: 'simulated',
      },
    })

    await expect(startLiveStrategy({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'btc-usdt-swap',
      inst_type: 'swap',
      timeframe: '15m',
      risk_timeframe: '1m',
      initial_capital: 1000,
      position_size: 0.25,
      stop_loss: 0,
      take_profit: 0,
      check_interval: 30.7,
      mode: 'simulated',
      params: {
        entry_use_high: false,
        enable_btc: true,
      },
    })).resolves.toMatchObject({
      status: 'running',
      running: true,
      run_id: 'run-v20',
      strategy_id: 'multi_timeframe_dual_v12',
      mode: 'simulated',
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/live/start',
        params: undefined,
        body: {
          strategy_id: 'multi_timeframe_dual_v12',
          symbol: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          risk_timeframe: '1m',
          initial_capital: 1000,
          position_size: 0.25,
          stop_loss: 0,
          take_profit: 0,
          check_interval: 31,
          mode: 'simulated',
          params: expect.objectContaining({
            entry_use_high: false,
            enable_btc: true,
          }),
        },
      },
    })
  })

  it('启动策略时拒绝已删除的本地组合层参数', () => {
    expect(() => startLiveStrategy({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      mode: 'simulated',
      params: {
        portfolio_layers: [{ id: 'btc_15m' }],
      },
    })).toThrow('portfolio_layers 本地组合架构')

    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('启动策略时拒绝非对象 params，避免静默丢弃非法运行参数', () => {
    expect(() => startLiveStrategy({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      mode: 'simulated',
      params: ['bad'],
    } as unknown as Parameters<typeof startLiveStrategy>[0])).toThrow('params 必须是 JSON 对象')

    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('启动策略时拒绝旧 paper 或未知运行模式', () => {
    for (const mode of ['paper', 'demo', 'simulation', 'sandbox']) {
      expect(() => startLiveStrategy({
        strategy_id: 'multi_timeframe_dual_v12',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        mode,
      })).toThrow('运行模式只支持 live 或 simulated')
    }

    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('决策诊断请求携带实时 K 线覆盖当前未收盘进度', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        summary: '开仓动作可执行',
        candle_count: 9501,
        realtime_candle_applied: true,
        decision_protocol: 'actions_v1',
        actions: [
          {
            action: 'open_position',
            symbol: 'BTC-USDT-SWAP',
            side: 'buy',
            order_type: 'market',
            price: 102,
            reference_price: 102,
            reason: 'candidate',
            strength: 0.8,
            timestamp: 1_780_000_900_000,
            position_size: 0.25,
            planned_exit_time: 1_780_001_800_000,
            planned_exit_reason: 'hold_bars_elapsed',
            planned_exit_contract: 'planned_exit_time_v1',
            source_index: 178,
            source_time: 1_780_000_000_000,
            feature_bar_time: 1_780_000_000_000,
            entry_time: 1_780_000_900_000,
            planned_hold_bars: 1,
            hold_bars: 1,
            layer_id: 'layer_topk',
            family: 'ml_selector',
            timeframe: '15m',
            candidate_source: 'ranked_candidate',
            candidate_entry_price: 101.5,
          },
        ],
        action_summary: {
          open_position: 1,
          close_position: 0,
          place_risk_order: 0,
          cancel_order: 0,
          modify_order: 0,
          hold: 0,
          total: 1,
        },
        execution_logs: [
          {
            stage: 'strategy',
            level: 'info',
            message: 'selected candidate',
            details: { symbol: 'BTC-USDT-SWAP' },
          },
        ],
        selected_symbols: ['BTC-USDT-SWAP'],
        blocked_by: [],
        execution_decision: {
          verdict: 'ready',
          summary: '可以提交订单',
          executable_intent_count: 1,
          risk_action_count: 0,
          skipped_action_count: 0,
          idle_action_count: 0,
          skipped_actions: [],
          gates: [
            {
              key: 'actions',
              label: '策略动作',
              status: 'pass',
              passed: true,
              blocking: false,
              detail: '返回 1 个可执行动作',
            },
          ],
        },
      },
    })

    const diagnostics = await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'btc-usdt-swap',
      inst_type: 'swap',
      timeframe: '15m',
      initial_capital: 1000,
      position_size: 0.25,
      stop_loss: 0,
      take_profit: 0,
      latest_candle: {
        inst_id: 'btc-usdt-swap',
        inst_type: 'swap',
        timeframe: '15m',
        timestamp: 1_780_000_900_000,
        open: 100,
        high: 103,
        low: 99,
        close: 102,
        volume: 12,
        volume_ccy: 1224,
        volume_quote: 1224,
        confirm: '0',
      },
    })

    expect(diagnostics.realtime_candle_applied).toBe(true)
    expect(diagnostics.decision_protocol).toBe('actions_v1')
    expect(diagnostics.actions[0]).toMatchObject({
      action: 'open_position',
      symbol: 'BTC-USDT-SWAP',
      side: 'buy',
      price: 102,
      position_size: 0.25,
      planned_exit_time: 1_780_001_800_000,
      planned_exit_reason: 'hold_bars_elapsed',
      planned_exit_contract: 'planned_exit_time_v1',
      source_index: 178,
      source_time: 1_780_000_000_000,
      feature_bar_time: 1_780_000_000_000,
      entry_time: 1_780_000_900_000,
      planned_hold_bars: 1,
      hold_bars: 1,
      layer_id: 'layer_topk',
      family: 'ml_selector',
      action_timeframe: '15m',
      candidate_source: 'ranked_candidate',
      candidate_entry_price: 101.5,
    })
    expect(diagnostics.action_summary.open_position).toBe(1)
    expect(diagnostics.execution_logs[0]).toMatchObject({
      stage: 'strategy',
      level: 'info',
      message: 'selected candidate',
    })
    expect(diagnostics.execution_decision?.verdict).toBe('ready')
    expect(diagnostics.execution_decision?.executable_intent_count).toBe(1)
    expect(diagnostics.execution_decision?.gates[0]).toMatchObject({
      key: 'actions',
      status: 'pass',
      blocking: false,
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/live/decision-diagnostics',
        params: undefined,
        body: expect.objectContaining({
          strategy_id: 'multi_timeframe_dual_v12',
          symbol: 'BTC-USDT-SWAP',
          inst_type: 'SWAP',
          timeframe: '15m',
          latest_candle: {
            inst_id: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            timeframe: '15m',
            timestamp: 1_780_000_900_000,
            open: 100,
            high: 103,
            low: 99,
            close: 102,
            volume: 12,
            volume_ccy: 1224,
            volume_quote: 1224,
            confirm: '0',
          },
        }),
      },
    })
  })

  it('决策诊断请求不把非法实时 K 线时间戳夹成有效值', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        actions: [],
      },
    })

    await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'btc-usdt-swap',
      inst_type: 'swap',
      timeframe: '15m',
      latest_candle: {
        inst_id: 'btc-usdt-swap',
        inst_type: 'swap',
        timeframe: '15m',
        timestamp: -100,
        open: 100,
        high: 103,
        low: 99,
        close: 102,
        volume: 12,
        volume_ccy: 1224,
        volume_quote: 1224,
        confirm: '0',
      },
    })

    expect(invokeMock).toHaveBeenCalledWith('local_api_request', {
      req: {
        method: 'POST',
        path: '/api/live/decision-diagnostics',
        params: undefined,
        body: expect.objectContaining({
          latest_candle: expect.not.objectContaining({
            timestamp: 1,
          }),
        }),
      },
    })
  })

  it('决策诊断请求拒绝旧 paper 运行模式', () => {
    for (const mode of ['paper', 'demo', 'simulation']) {
      expect(() => fetchDecisionDiagnostics({
        strategy_id: 'multi_timeframe_dual_v12',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        mode,
      })).toThrow('运行模式只支持 live 或 simulated')
    }

    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('决策诊断请求拒绝非对象 params，避免诊断与启动参数不一致', () => {
    expect(() => fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
      mode: 'simulated',
      params: ['bad'],
    } as unknown as Parameters<typeof fetchDecisionDiagnostics>[0])).toThrow('params 必须是 JSON 对象')

    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('决策诊断不再从旧日志和预览别名补 canonical 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        decision: { protocol: 'actions_v1' },
        actions: [],
        execution_logs: [{
          level: 'warning',
          message: 'legacy log level',
          details: {},
        }],
        execution_decision: {
          verdict: 'ready',
        },
      },
    })

    const diagnostics = await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
    })

    expect(diagnostics.execution_logs[0]).toMatchObject({
      stage: '',
      level: '',
      message: 'legacy log level',
    })
    expect(diagnostics.decision_protocol).toBe('')
    expect(diagnostics.execution_decision?.verdict).toBe('ready')
    expect(diagnostics.execution_decision?.executable_intent_count).toBe(0)
  })

  it('决策诊断 action 不从 OKX 原始字段 fallback', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        actions: [{
          action: 'open_position',
          inst_id: 'ETH-USDT-SWAP',
          pos_side: 'long',
          ord_type: 'limit',
          error_message: 'stale action fallback',
          timestamp: 1_780_000_900_000,
        }],
        action_summary: {
          open_position: 1,
          total: 1,
        },
      },
    })

    const diagnostics = await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
    })

    expect(diagnostics.actions[0]).toMatchObject({
      action: 'open_position',
      symbol: '',
      side: '',
      order_type: '',
      reason: '',
    })
  })

  it('决策诊断保留订单管理 action 字段', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        actions: [{
          action: 'modify_order',
          symbol: 'BTC-USDT-SWAP',
          side: 'hold',
          order_type: 'market',
          order_side: 'sell',
          close_side: 'long',
          order_id: 'algo-order-1',
          client_order_id: 'algo-client-1',
          new_size: '2',
          new_price: '94.5',
          request_id: 'modify-req-1',
          cancel_on_fail: true,
          target_order_kind: 'algo',
          target_order_type: 'stop_market',
          timestamp: 1_780_000_900_000,
        }],
        action_summary: {
          modify_order: 1,
          total: 1,
        },
      },
    })

    const diagnostics = await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
    })

    expect(diagnostics.actions[0]).toMatchObject({
      action: 'modify_order',
      symbol: 'BTC-USDT-SWAP',
      order_side: 'sell',
      close_side: 'long',
      order_id: 'algo-order-1',
      client_order_id: 'algo-client-1',
      new_size: '2',
      new_price: '94.5',
      request_id: 'modify-req-1',
      cancel_on_fail: true,
      target_order_kind: 'algo',
      target_order_type: 'stop_market',
    })
  })

  it('决策诊断响应不把字符串数字当作有效动作数值', async () => {
    invokeMock.mockResolvedValueOnce({
      code: 0,
      data: {
        strategy_id: 'multi_timeframe_dual_v12',
        strategy_name: 'V20',
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        timeframe: '15m',
        candle_count: '160',
        actions: [
          {
            action: 'open_position',
            symbol: 'BTC-USDT-SWAP',
            side: 'buy',
            order_type: 'limit',
            price: '102',
            reference_price: '102',
            trigger_price: '98',
            reason: 'candidate',
            strength: '0.8',
            timestamp: '1780000900000',
            position_size: '0.25',
            planned_exit_time: '1780001800000',
            source_index: '178',
            source_time: '1780000000000',
            feature_bar_time: '1780000000000',
            entry_time: '1780000900000',
            planned_hold_bars: '1',
            hold_bars: '1',
            candidate_entry_price: '101.5',
          },
        ],
        action_summary: {
          open_position: '1',
          total: '1',
        },
        execution_decision: {
          verdict: 'ready',
          summary: '可以提交订单',
          executable_intent_count: '1',
          risk_action_count: '0',
          skipped_action_count: '1',
          idle_action_count: '0',
          skipped_actions: [],
          gates: [],
        },
      },
    })

    const diagnostics = await fetchDecisionDiagnostics({
      strategy_id: 'multi_timeframe_dual_v12',
      symbol: 'BTC-USDT-SWAP',
      inst_type: 'SWAP',
      timeframe: '15m',
    })

    expect(diagnostics.candle_count).toBeNull()
    expect(diagnostics.actions[0]).toMatchObject({
      price: null,
      reference_price: null,
      trigger_price: null,
      strength: null,
      timestamp: 0,
      position_size: null,
      planned_exit_time: null,
      source_index: null,
      source_time: null,
      feature_bar_time: null,
      entry_time: null,
      planned_hold_bars: null,
      hold_bars: null,
      candidate_entry_price: null,
    })
    expect(diagnostics.action_summary.open_position).toBe(0)
    expect(diagnostics.action_summary.total).toBe(0)
    expect(diagnostics.execution_decision?.executable_intent_count).toBe(0)
  })
})
