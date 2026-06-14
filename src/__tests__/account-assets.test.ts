import { describe, expect, it } from 'vitest'
import type { AccountAsset, AccountInfo } from '@/types'
import {
  accountTotalEquityUsd,
  assetAvailableUsd,
  assetEquityUsd,
  assetFrozenUsd,
  formatAssetUsd,
} from '@/utils/accountAssets'

describe('账户资产 USD 口径', () => {
  it('按 OKX 资产页用 eqUsd 折算权益、占用和可用', () => {
    const btc = asset({
      ccy: 'BTC',
      total: 1,
      available: 0.99999986,
      frozen: 0,
      eq: 1,
      eq_usd: 73390.65,
    })

    expect(assetEquityUsd(btc)).toBeCloseTo(73390.65)
    expect(assetFrozenUsd(btc)).toBe(0)
    expect(assetAvailableUsd(btc)).toBeCloseTo(73390.64, 1)
    expect(formatAssetUsd(assetEquityUsd(btc))).toBe('$73,390.65')
  })

  it('实盘缺 eqUsd 时回退到 disEq，不把非 USDT 显示成 raw 币种数量', () => {
    const okb = asset({
      ccy: 'OKB',
      total: 100,
      available: 100,
      eq: 100,
      eq_usd: 0,
      dis_eq: 8632.58,
    })

    expect(assetEquityUsd(okb)).toBeCloseTo(8632.58)
    expect(assetAvailableUsd(okb)).toBeCloseTo(8632.58)
  })

  it('账户总资产优先使用 OKX totalEq，缺失时才汇总明细', () => {
    const account: AccountInfo = {
      total_eq: 89001.85,
      iso_eq: 0,
      adj_eq: 0,
      usdt_balance: 4990.51,
      usdt_available: 4990.51,
      usdt_equity_usd: 4990.51,
      details: [
        asset({ ccy: 'BTC', total: 1, eq: 1, eq_usd: 73390.65 }),
        asset({ ccy: 'USDT', total: 4990.51, available: 4990.51, eq: 4990.51 }),
      ],
    }

    expect(accountTotalEquityUsd(account)).toBe(89001.85)
  })

  it('非稳定币缺 USD 估值时保留 unknown，不显示成 0 美元', () => {
    const unknownAlt = asset({
      ccy: 'DOGE',
      total: 2,
      available: 1,
      eq: 2,
      eq_usd: null,
      dis_eq: null,
    })

    expect(assetEquityUsd(unknownAlt)).toBeNull()
    expect(assetAvailableUsd(unknownAlt)).toBeNull()
    expect(formatAssetUsd(assetEquityUsd(unknownAlt))).toBe('--')

    const unknownAccount: AccountInfo = {
      total_eq: null,
      iso_eq: null,
      adj_eq: null,
      usdt_balance: null,
      usdt_available: null,
      usdt_equity_usd: null,
      details: [unknownAlt],
    }
    const zeroAccount: AccountInfo = {
      total_eq: 0,
      iso_eq: 0,
      adj_eq: 0,
      usdt_balance: 0,
      usdt_available: 0,
      usdt_equity_usd: 0,
      details: [],
    }

    expect(accountTotalEquityUsd(unknownAccount)).toBeNull()
    expect(accountTotalEquityUsd(zeroAccount)).toBe(0)
  })
})

function asset(overrides: Partial<AccountAsset>): AccountAsset {
  return {
    ccy: '',
    total: 0,
    available: 0,
    frozen: 0,
    cash_bal: 0,
    avail_bal: 0,
    avail_eq: 0,
    eq: 0,
    eq_usd: 0,
    dis_eq: 0,
    ord_frozen: 0,
    u_time: 0,
    ...overrides,
  }
}
