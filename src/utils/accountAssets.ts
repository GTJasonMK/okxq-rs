import type { AccountAsset, AccountInfo } from '@/types'

export function accountTotalEquityUsd(account: AccountInfo | null): number | null {
  if (!account) return 0
  if (isNonZeroFinite(account.total_eq)) return account.total_eq
  const detailsTotal = sumKnown(account.details.map(assetEquityUsd))
  return detailsTotal ?? finiteOrNull(account.total_eq)
}

export function assetEquityUsd(asset: AccountAsset): number | null {
  if (isNonZeroFinite(asset.eq_usd)) return asset.eq_usd
  if (isNonZeroFinite(asset.dis_eq)) return asset.dis_eq
  if (isUsdStablecoin(asset.ccy)) return finiteOrNull(asset.total)
  return null
}

export function assetAvailableUsd(asset: AccountAsset): number | null {
  return assetAmountUsd(asset, asset.available)
}

export function assetFrozenUsd(asset: AccountAsset): number | null {
  return assetAmountUsd(asset, firstNonZero(asset.frozen, asset.ord_frozen))
}

export function hasVisibleAssetBalance(asset: AccountAsset): boolean {
  return [
    asset.total,
    asset.available,
    asset.frozen,
    assetEquityUsd(asset),
  ].some(isNonZeroFinite)
}

export function formatAssetUsd(value: number | null | undefined): string {
  if (!isFiniteNumber(value)) return '--'
  const prefix = value < 0 ? '-' : ''
  const abs = Math.abs(value)
  return `${prefix}$${abs.toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`
}

function assetAmountUsd(asset: AccountAsset, amount: number | null): number | null {
  if (!isFiniteNumber(amount)) return null
  if (Math.abs(amount) === 0) return 0
  const equityUsd = assetEquityUsd(asset)
  if (isNonZeroFinite(asset.eq) && isNonZeroFinite(equityUsd)) {
    return (amount / asset.eq) * equityUsd
  }
  if (isNonZeroFinite(asset.total) && isNonZeroFinite(equityUsd)) {
    return (amount / asset.total) * equityUsd
  }
  if (isUsdStablecoin(asset.ccy)) return amount
  return null
}

function firstNonZero(...values: Array<number | null>): number | null {
  let firstFinite: number | null = null
  for (const value of values) {
    if (!isFiniteNumber(value)) continue
    firstFinite = value
    if (Math.abs(value) > 0) return value
  }
  return firstFinite
}

function finiteOrNull(value: number | null): number | null {
  return isFiniteNumber(value) ? value : null
}

function isNonZeroFinite(value: number | null): value is number {
  return isFiniteNumber(value) && Math.abs(value) > 0
}

function sumKnown(values: Array<number | null>): number | null {
  let found = false
  let total = 0
  for (const value of values) {
    if (!isFiniteNumber(value)) continue
    found = true
    total += value
  }
  return found ? total : null
}

function isUsdStablecoin(ccy: string): boolean {
  return ['USDT', 'USDC', 'USD', 'DAI', 'TUSD', 'FDUSD'].includes(ccy.trim().toUpperCase())
}

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}
