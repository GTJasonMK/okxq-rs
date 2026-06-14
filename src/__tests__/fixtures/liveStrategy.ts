import type {
  LiveExecutionPlan,
  LiveOrder,
  LiveEquityHistory,
  LiveDecisionDiagnostics,
  LiveStrategyStatus,
} from '@/types'

export function status(overrides: Partial<LiveStrategyStatus> = {}): LiveStrategyStatus {
  return {
    status: 'stopped',
    running: false,
    run_id: 'run',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'Multi-Timeframe Overextension',
    symbol: 'BTC-USDT-SWAP',
    timeframe: '15m',
    inst_type: 'SWAP',
    risk_timeframe: '1m',
    mode: 'simulated',
    initial_capital: 1000,
    position_size: 0.25,
    stop_loss: 0,
    take_profit: 0,
    params: {},
    start_time: null,
    last_action_time: null,
    last_action: '',
    actions_generated: 0,
    orders_placed: 0,
    successful_orders: 0,
    failed_orders: 0,
    error_message: '',
    check_interval: 60,
    execution_mode: 'exchange_demo',
    last_price: 0,
    last_action_strength: 0,
    last_action_reason: '',
    last_order_candle_ts: 0,
    ...overrides,
  }
}

export function equityHistory(overrides: Partial<LiveEquityHistory> = {}): LiveEquityHistory {
  return {
    run_id: 'run',
    mode: 'simulated',
    count: 0,
    snapshots: [],
    daily: [],
    ...overrides,
  }
}

export function equitySnapshot(overrides: Partial<LiveEquityHistory['snapshots'][number]> = {}): LiveEquityHistory['snapshots'][number] {
  return {
    id: 1,
    run_id: 'run',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    symbol: 'BTC-USDT-SWAP',
    inst_id: 'BTC-USDT-SWAP',
    timeframe: '15m',
    inst_type: 'SWAP',
    mode: 'simulated',
    timestamp: 1_780_000_000_000,
    time: '2026-05-28 00:00:00',
    trading_day: '2026-05-28',
    price: 100,
    position_side: 'flat',
    entry_price: 0,
    quantity: 0,
    initial_capital: 1000,
    day_start_equity: 1000,
    equity: 1000,
    realized_pnl: 0,
    unrealized_pnl: 0,
    total_pnl: 0,
    total_pnl_pct: 0,
    today_pnl: 0,
    today_pnl_pct: 0,
    created_at: 1_780_000_000_000,
    ...overrides,
  }
}

export function liveOrder(overrides: Partial<LiveOrder> = {}): LiveOrder {
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
    px: 100,
    reference_price: null,
    reference_price_source: '',
    reference_price_missing: false,
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
    action: 'open_position',
    success: true,
    status: 'filled',
    error_message: '',
    mode: 'simulated',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    run_id: 'run',
    timestamp: 1_780_000_000_000,
    arrival_ts: null,
    arrival_mid_px: null,
    arrival_bid_px: null,
    arrival_ask_px: null,
    created_at: 1_780_000_000_000,
    ...overrides,
  }
}

export function liveExecutionPlan(overrides: Partial<LiveExecutionPlan> = {}): LiveExecutionPlan {
  return {
    id: 1,
    plan_key: 'plan-key',
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    mode: 'simulated',
    entry_run_id: 'run',
    exit_run_id: '',
    symbol: 'BTC-USDT-SWAP',
    inst_id: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    entry_order_id: 'entry-order-id',
    entry_client_order_id: 'entry-client-order-id',
    entry_timestamp: 1_780_000_000_000,
    entry_side: 'buy',
    entry_price: 100,
    close_side: 'sell',
    planned_exit_time: 1_780_000_900_000,
    planned_exit_reason: 'hold_bars_elapsed',
    planned_exit_contract: 'planned_exit_time_v1',
    status: 'scheduled',
    exit_order_id: '',
    exit_client_order_id: '',
    attempt_count: 0,
    next_attempt_at: null,
    last_error: '',
    created_at: 1_780_000_000_000,
    updated_at: 1_780_000_000_000,
    ...overrides,
  }
}

export function decisionDiagnostics(overrides: Partial<LiveDecisionDiagnostics> = {}): LiveDecisionDiagnostics {
  return {
    strategy_id: 'multi_timeframe_dual_v12',
    strategy_name: 'V20',
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP' as const,
    timeframe: '15m' as const,
    summary: '策略当前未返回动作',
    candle_count: 9501,
    realtime_candle_applied: false,
    decision_protocol: 'actions_v1',
    actions: [],
    action_summary: {
      open_position: 0,
      close_position: 0,
      place_risk_order: 0,
      cancel_order: 0,
      modify_order: 0,
      hold: 0,
      total: 0,
    },
    execution_logs: [],
    selected_symbols: ['BTC-USDT-SWAP'],
    blocked_by: [],
    execution_decision: {
      verdict: 'hold',
      summary: '策略当前未返回动作',
      executable_intent_count: 0,
      risk_action_count: 0,
      skipped_action_count: 0,
      idle_action_count: 0,
      skipped_actions: [],
      gates: [
        {
          key: 'runtime',
          label: '运行状态',
          status: 'pass',
          passed: true,
          blocking: false,
          detail: '当前运行目标一致',
        },
      ],
    },
    raw: {},
    ...overrides,
  }
}

export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}
