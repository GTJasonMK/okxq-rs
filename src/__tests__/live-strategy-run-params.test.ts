import { describe, expect, it } from 'vitest'
import {
  buildParamsFromDraftRows,
  omitEngineParams,
} from '@/utils/backtestResultCard'
import { DEFAULT_LIVE_CONTROL_FORM } from '@/utils/liveStrategyControl'
import {
  liveRunRuntimeDraftRows,
  liveRunStrategyDraftRows,
} from '@/utils/liveStrategyRunParams'
import type { StrategyMeta } from '@/types'

describe('liveStrategyRunParams', () => {
  it('SWAP 策略启动参数始终提供合约模式、保证金模式和杠杆入口', () => {
    const form = {
      ...DEFAULT_LIVE_CONTROL_FORM,
      symbol: 'BTC-USDT-SWAP',
      params: {},
    }

    const rows = liveRunRuntimeDraftRows(form, strategyMeta())

    expect(rowByKey(rows, 'contract_mode')).toMatchObject({
      label: '合约模式',
      input: 'true',
      kind: 'boolean',
    })
    expect(rowByKey(rows, 'td_mode')).toMatchObject({
      label: '保证金模式',
      input: 'cross',
      kind: 'string',
    })
    expect(rowByKey(rows, 'leverage')).toMatchObject({
      label: '杠杆倍数',
      input: '1',
      kind: 'number',
    })
  })

  it('FUTURES 策略也提供合约保证金和杠杆入口，不依赖 symbol 后缀推断', () => {
    const form = {
      ...DEFAULT_LIVE_CONTROL_FORM,
      symbol: 'BTC-USDT-260626',
      inst_type: 'FUTURES' as const,
      params: {},
    }

    const rows = liveRunRuntimeDraftRows(
      form,
      strategyMeta({
        runtime: {
          ...runtimeMeta(),
          symbol: 'BTC-USDT-260626',
          inst_type: 'FUTURES',
        },
      }),
    )

    expect(rowByKey(rows, 'contract_mode')).toMatchObject({ input: 'true' })
    expect(rowByKey(rows, 'td_mode')).toMatchObject({ input: 'cross' })
    expect(rowByKey(rows, 'leverage')).toMatchObject({ input: '1' })
  })

  it('启动参数草稿会把交易执行参数提交到 params 而不是策略私有参数区', () => {
    const form = {
      ...DEFAULT_LIVE_CONTROL_FORM,
      initial_capital: 100,
      params: {
        leverage: 3,
        td_mode: 'isolated',
        max_slippage_bps: 20,
        live_fill_sync_symbol_limit: 5,
        custom_threshold: 0.7,
      },
    }
    const strategyRows = liveRunStrategyDraftRows(form)
    const runtimeRows = liveRunRuntimeDraftRows(form, strategyMeta())

    rowByKey(runtimeRows, 'leverage').input = '5'
    rowByKey(runtimeRows, 'td_mode').input = 'cross'
    rowByKey(runtimeRows, 'max_slippage_bps').input = '15'

    expect(rowByKey(runtimeRows, 'initial_capital')).toMatchObject({
      label: '初始资金',
      input: '100',
      kind: 'number',
    })
    expect(strategyRows.map(row => row.key)).toEqual(['custom_threshold'])
    expect(buildParamsFromDraftRows(strategyRows, 'strict')).toEqual({
      custom_threshold: 0.7,
    })
    expect(buildParamsFromDraftRows(runtimeRows, 'strict')).toMatchObject({
      initial_capital: 100,
      leverage: 5,
      td_mode: 'cross',
      contract_mode: true,
      max_slippage_bps: 15,
      live_fill_sync_symbol_limit: 5,
    })
    expect(omitEngineParams(form.params)).toEqual({
      custom_threshold: 0.7,
    })
  })

  it('SPOT 策略不会显示合约执行参数', () => {
    const rows = liveRunRuntimeDraftRows(
      {
        ...DEFAULT_LIVE_CONTROL_FORM,
        symbol: 'BTC-USDT',
        params: {},
      },
      strategyMeta({
        runtime: {
          ...runtimeMeta(),
          symbol: 'BTC-USDT',
          inst_type: 'SPOT',
        },
      }),
    )

    expect(rows.some(row => row.key === 'contract_mode')).toBe(false)
    expect(rows.some(row => row.key === 'td_mode')).toBe(false)
    expect(rows.some(row => row.key === 'leverage')).toBe(false)
  })
})

function rowByKey<T extends { key: string }>(rows: T[], key: string): T {
  const row = rows.find(item => item.key === key)
  expect(row).toBeTruthy()
  return row as T
}

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy',
    name: 'Strategy',
    description: '',
    runtime: runtimeMeta(),
    visualization: {},
    decision_contract: {},
    ...overrides,
  }
}

function runtimeMeta(): NonNullable<StrategyMeta['runtime']> {
  return {
    symbol: 'BTC-USDT-SWAP',
    inst_type: 'SWAP',
    timeframe: '15m',
    risk_timeframe: '1m',
    initial_capital: 1000,
    position_size: 0.25,
    stop_loss: 0,
    take_profit: 0,
    check_interval: 60,
    mode: 'simulated',
    params: {},
  }
}
