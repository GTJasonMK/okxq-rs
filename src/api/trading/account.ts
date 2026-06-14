import type * as T from '@/types/trading'
import { okxNullableNumberValue as numericValue } from '../okxPayload'
import {
  arrayRecords,
  isRecord,
  nullableNumberValue,
  recordFrom,
  stringValue,
  timestampNumber as timestampValue,
} from '../normalize'

type AccountNumberReader = (value: unknown) => number | null
interface AccountAssetNormalizeOptions {
  readNumber: AccountNumberReader
  readRaw: boolean
}

export function normalizeAccount(raw: unknown): T.AccountInfo {
  const item = recordFrom(raw)
  const details = arrayRecords(item.details)
    .map(item => normalizeAccountAsset(item, {
      readNumber: nullableNumberValue,
      readRaw: false,
    }))
    .filter(hasAssetBalance)
  const usdtBalance = nullableNumberValue(item.usdt_balance)
  return {
    total_eq: nullableNumberValue(item.total_eq),
    iso_eq: nullableNumberValue(item.iso_eq),
    adj_eq: nullableNumberValue(item.adj_eq),
    usdt_balance: usdtBalance,
    usdt_available: hasOwn(item, 'usdt_available')
      ? nullableNumberValue(item.usdt_available)
      : usdtBalance,
    usdt_equity_usd: hasOwn(item, 'usdt_equity_usd')
      ? nullableNumberValue(item.usdt_equity_usd)
      : usdtBalance,
    details,
  }
}

export function normalizePrivateAccountEvent(raw: unknown): T.AccountInfo | null {
  const payload = recordFrom(raw)
  const data = recordFrom(payload.data)
  const details = Object.values(data).filter(isRecord)
  const account = recordFrom(payload.account)
  if (details.length === 0 && Object.keys(account).length === 0) return null

  const eventAccountOptions: AccountAssetNormalizeOptions = {
    readNumber: numericValue,
    readRaw: true,
  }

  const fallbackTotalEq = sumKnown(details.map(detail => accountDetailNumber(detail, [
    'eqUsd',
    'eq_usd',
    'disEq',
    'dis_eq',
    'eq',
  ], eventAccountOptions)))
  const totalEq = numericValue(account.total_eq)
    ?? numericValue(account.total_equity)
    ?? numericValue(account.totalEq)
    ?? fallbackTotalEq
  const usdt = details.find(detail => stringValue(detail.ccy).trim().toUpperCase() === 'USDT')
  const usdtBalance = usdt ? accountDetailNumber(usdt, [
    'cashBal',
    'cash_bal',
    'eq',
    'availBal',
    'avail_bal',
    'availEq',
    'avail_eq',
  ], eventAccountOptions) : null
  const usdtAvailable = usdt ? accountDetailNumber(usdt, [
    'availBal',
    'avail_bal',
    'availEq',
    'avail_eq',
    'cashBal',
    'cash_bal',
    'eq',
  ], eventAccountOptions) : null
  const usdtEquityUsd = usdt ? accountDetailNumber(usdt, [
    'eqUsd',
    'eq_usd',
    'disEq',
    'dis_eq',
  ], eventAccountOptions) : null
  const assets = details
    .map(item => normalizeAccountAsset(item, eventAccountOptions))
    .filter(hasAssetBalance)

  return {
    total_eq: totalEq,
    iso_eq: numericValue(account.iso_eq) ?? numericValue(account.isoEq),
    adj_eq: numericValue(account.adj_eq) ?? numericValue(account.adjEq),
    usdt_balance: usdtBalance,
    usdt_available: usdtAvailable,
    usdt_equity_usd: usdtEquityUsd,
    details: assets,
  }
}

function normalizeAccountAsset(
  item: Record<string, unknown>,
  options: AccountAssetNormalizeOptions,
): T.AccountAsset {
  const cashBal = accountDetailNumber(item, ['cash_bal', 'cashBal'], options)
  const availBal = accountDetailNumber(item, ['avail_bal', 'availBal'], options)
  const availEq = accountDetailNumber(item, ['avail_eq', 'availEq'], options)
  const frozenBal = accountDetailNumber(item, ['frozen_bal', 'frozenBal'], options)
  const ordFrozen = accountDetailNumber(item, ['ord_frozen', 'ordFrozen', 'ordFroz'], options)
  const eq = accountDetailNumber(item, ['eq'], options)
  const eqUsd = accountDetailNumber(item, ['eq_usd', 'eqUsd'], options)
  const disEq = accountDetailNumber(item, ['dis_eq', 'disEq'], options)
  const total = firstNumericOrNull([
    item.total,
    cashBal,
    eq,
    sumKnown([availBal, firstNumericOrNull([frozenBal, ordFrozen], options.readNumber)]),
  ], options.readNumber)
  const available = firstNumericOrNull([
    item.available,
    availBal,
    availEq,
    cashBal,
  ], options.readNumber)
  return {
    ccy: stringValue(item.ccy).trim().toUpperCase(),
    total,
    available,
    frozen: firstNumericOrNull([item.frozen, frozenBal, ordFrozen], options.readNumber),
    cash_bal: cashBal,
    avail_bal: availBal,
    avail_eq: availEq,
    eq,
    eq_usd: eqUsd,
    dis_eq: disEq,
    ord_frozen: ordFrozen,
    u_time: timestampValue(item.u_time ?? item.uTime, 0),
  }
}

function hasAssetBalance(asset: T.AccountAsset): boolean {
  return Boolean(asset.ccy) && [
    asset.total,
    asset.available,
    asset.frozen,
    asset.cash_bal,
    asset.avail_bal,
    asset.avail_eq,
    asset.eq,
    asset.eq_usd,
    asset.dis_eq,
    asset.ord_frozen,
  ].some(isNonZeroFinite)
}

function firstNumericOrNull(values: unknown[], readNumber: AccountNumberReader): number | null {
  let firstParsed: number | null = null
  for (const value of values) {
    const parsed = readNumber(value)
    if (parsed === null) continue
    firstParsed ??= parsed
    if (Math.abs(parsed) > 0) return parsed
  }
  return firstParsed
}

function sumKnown(values: Array<number | null>): number | null {
  let found = false
  let total = 0
  for (const value of values) {
    if (!Number.isFinite(value)) continue
    found = true
    total += value as number
  }
  return found ? total : null
}

function accountDetailNumber(
  item: Record<string, unknown>,
  keys: string[],
  options: AccountAssetNormalizeOptions,
) {
  for (const key of keys) {
    const parsed = options.readNumber(item[key])
    if (parsed !== null) return parsed
  }
  if (!options.readRaw) return null
  const raw = recordFrom(item.raw)
  for (const key of keys) {
    const parsed = options.readNumber(raw[key])
    if (parsed !== null) return parsed
  }
  return null
}

function hasOwn(item: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(item, key)
}

function isNonZeroFinite(value: number | null): boolean {
  return Number.isFinite(value) && Math.abs(value as number) > 0
}
