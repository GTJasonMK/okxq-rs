import { describe, expect, it } from 'vitest'
import {
  buildParamsFromDraftRows,
  draftRowFromValue,
  draftRowsFromParams,
  engineParamSpecsFromSource,
  omitEngineParams,
  pickEngineParams,
  readableParamRows,
} from '@/utils/backtestResultCard'

describe('backtestResultCard utils', () => {
  it('将引擎参数展平为可读行并按字段类型格式化', () => {
    const rows = readableParamRows({
      cost_model: {
        fee_rate: 0.0005,
        position_size: 0.2,
        leverage: 3,
        allow_short: true,
        position_size_mode: 'margin_fraction',
        total_funding: -1.25,
      },
      execution_model: {
        timing: 'next_open',
        delay_bars: 1,
      },
    })

    expect(rowByKey(rows, 'cost_model')).toMatchObject({
      label: '费用模型',
      group: true,
    })
    expect(rowByKey(rows, 'cost_model.fee_rate')).toMatchObject({
      label: '手续费率',
      value: '0.000500 (0.0500%)',
    })
    expect(rowByKey(rows, 'cost_model.position_size')).toMatchObject({
      label: '仓位大小',
      value: '0.2000 (20.00%)',
    })
    expect(rowByKey(rows, 'cost_model.leverage')).toMatchObject({
      label: '杠杆倍数',
      value: '3x',
    })
    expect(rowByKey(rows, 'cost_model.allow_short')).toMatchObject({
      label: '允许做空',
      value: '是',
    })
    expect(rowByKey(rows, 'cost_model.position_size_mode')).toMatchObject({
      label: '仓位大小模式',
      value: '按保证金比例',
    })
    expect(rowByKey(rows, 'cost_model.total_funding')).toMatchObject({
      label: '累计资金费',
      value: '-1.25',
    })
    expect(rowByKey(rows, 'execution_model.timing')).toMatchObject({
      label: '执行时机',
      value: '下一根K线开盘成交',
    })
    expect(rowByKey(rows, 'execution_model.delay_bars')).toMatchObject({
      label: '执行延迟K线数',
      value: '1 根',
    })
  })

  it('将策略参数草稿回填为原始嵌套结构', () => {
    const rows = draftRowsFromParams({
      risk: {
        max_leverage: 3,
      },
      strict_context_gating: false,
      labels: ['alpha'],
    })

    rowByKey(rows, 'risk.max_leverage').input = '4.5'
    rowByKey(rows, 'strict_context_gating').input = 'true'
    rowByKey(rows, 'labels').input = JSON.stringify(['beta'])

    expect(buildParamsFromDraftRows(rows, 'strict')).toEqual({
      risk: {
        max_leverage: 4.5,
      },
      strict_context_gating: true,
      labels: ['beta'],
    })
  })

  it('参数草稿无效时返回空结果并写回行错误', () => {
    const rows = draftRowsFromParams({
      leverage: 3,
    })

    rowByKey(rows, 'leverage').input = 'abc'

    expect(buildParamsFromDraftRows(rows, 'strict')).toBeNull()
    expect(rowByKey(rows, 'leverage').error).toBe('请输入有效数字')

    rowByKey(rows, 'leverage').input = '5'

    expect(buildParamsFromDraftRows(rows, 'strict')).toEqual({ leverage: 5 })
    expect(rowByKey(rows, 'leverage').error).toBe('')
  })

  it('引擎参数草稿跳过空值且只挑选引擎相关字段', () => {
    const rows = [
      draftRowFromValue({ key: 'fee_rate', label: '手续费率', value: undefined, depth: 0 }, 'number'),
      draftRowFromValue({ key: 'allow_short', label: '允许做空', value: undefined, depth: 0 }, 'boolean'),
      draftRowFromValue({ key: 'execution_timing', label: '执行时机', value: 'next_open', depth: 0 }, 'string'),
    ]
    rowByKey(rows, 'execution_timing').input = 'same_close'

    expect(buildParamsFromDraftRows(rows, 'skip-empty')).toEqual({
      execution_timing: 'same_close',
    })
    expect(pickEngineParams({
      backtest_instrument_rules_source: 'okx',
      ctVal: 0.01,
      fee_rate: 0.0005,
      lotSz: 0.01,
      stop_loss: 0.02,
      strict_context_gating: true,
      custom_signal_threshold: 0.7,
    })).toEqual({
      backtest_instrument_rules_source: 'okx',
      ctVal: 0.01,
      fee_rate: 0.0005,
      lotSz: 0.01,
      stop_loss: 0.02,
    })
    expect(omitEngineParams({
      backtest_instrument_rules_source: 'okx',
      ctVal: 0.01,
      fee_rate: 0.0005,
      lotSz: 0.01,
      stop_loss: 0.02,
      strict_context_gating: true,
      custom_signal_threshold: 0.7,
    })).toEqual({
      strict_context_gating: true,
      custom_signal_threshold: 0.7,
    })
  })

  it('回测引擎参数默认使用模拟交易规格并暴露手动/OKX来源选项', () => {
    const specs = engineParamSpecsFromSource({
      runtime: {
        symbol: 'BTC-USDT-SWAP',
        inst_type: 'SWAP',
        position_size: 0.2,
      },
    })

    expect(rowByKey(specs, 'backtest_instrument_rules_source')).toMatchObject({
      kind: 'select',
      value: 'simulated',
      options: [
        { label: '模拟规格', value: 'simulated' },
        { label: '手动参数', value: 'params' },
        { label: 'OKX 实时规格', value: 'okx' },
      ],
    })
    expect(rowByKey(specs, 'ctVal')).toMatchObject({ value: 1 })
    expect(rowByKey(specs, 'ctValCcy')).toMatchObject({ value: 'BTC' })
    expect(rowByKey(specs, 'lotSz')).toMatchObject({ value: 1 })
    expect(rowByKey(specs, 'minSz')).toMatchObject({ value: 1 })
    expect(rowByKey(specs, 'tickSz')).toMatchObject({ value: 0.00000001 })
  })

  it('回测交易规格来源会把后端支持的别名归一为下拉标准值', () => {
    expect(rowByKey(engineParamSpecsFromSource({
      params: { instrument_rules_source: 'manual' },
    }), 'backtest_instrument_rules_source')).toMatchObject({ value: 'params' })

    expect(rowByKey(engineParamSpecsFromSource({
      params: { historical_instrument_rules_source: 'exchange' },
    }), 'backtest_instrument_rules_source')).toMatchObject({ value: 'okx' })
  })

  it('select 参数只接受声明过的选项值', () => {
    const row = draftRowFromValue({
      key: 'backtest_instrument_rules_source',
      label: '交易规格来源',
      value: 'simulated',
      depth: 0,
    }, 'select')
    row.options = [
      { label: '模拟规格', value: 'simulated' },
      { label: '手动参数', value: 'params' },
      { label: 'OKX 实时规格', value: 'okx' },
    ]

    row.input = 'params'
    expect(buildParamsFromDraftRows([row], 'strict')).toEqual({
      backtest_instrument_rules_source: 'params',
    })

    row.input = 'unknown'
    expect(buildParamsFromDraftRows([row], 'strict')).toBeNull()
    expect(row.error).toBe('请选择有效选项')
  })
})

function rowByKey<T extends { key: string }>(rows: T[], key: string): T {
  const row = rows.find(item => item.key === key)
  expect(row).toBeTruthy()
  return row as T
}
