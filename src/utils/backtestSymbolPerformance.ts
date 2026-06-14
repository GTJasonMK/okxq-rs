import type { BacktestTrade } from '@/types'

export interface BacktestSymbolPerformanceRow {
  symbol: string
  baseSymbol: string
  closedTrades: number
  winningTrades: number
  losingTrades: number
  winRatePct: number
  realizedPnl: number
  capitalContributionPct: number
  turnoverReturnPct: number
  turnover: number
  commission: number
  longPnl: number
  shortPnl: number
}

interface MutableSymbolPerformance {
  symbol: string
  baseSymbol: string
  closedTrades: number
  winningTrades: number
  losingTrades: number
  realizedPnl: number
  turnover: number
  commission: number
  longPnl: number
  shortPnl: number
}

export function buildBacktestSymbolPerformance(
  trades: readonly BacktestTrade[],
  initialCapital = 0,
): BacktestSymbolPerformanceRow[] {
  const rows = new Map<string, MutableSymbolPerformance>()
  const normalizedSymbols = new Map<string | undefined, string>()

  for (const trade of trades) {
    const symbol = normalizedSymbol(trade.symbol, normalizedSymbols)
    if (!symbol) continue
    const row = ensureRow(rows, symbol)
    const commission = trade.commission
    if (Number.isFinite(commission) && commission > 0) {
      row.commission += commission
    }
    const action = normalizeLower(trade.action)
    if (!isRealizedTrade(trade, action)) continue

    const pnl = finiteNumber(trade.pnl)
    const turnover = tradeTurnover(trade)
    row.closedTrades += 1
    row.realizedPnl += pnl
    row.turnover += turnover
    if (pnl > 0) row.winningTrades += 1
    if (pnl < 0) row.losingTrades += 1

    if (positionSide(trade, action) === 'short') {
      row.shortPnl += pnl
    } else {
      row.longPnl += pnl
    }
  }

  const result: BacktestSymbolPerformanceRow[] = []
  for (const row of rows.values()) {
    if (row.closedTrades <= 0) continue
    result.push({
      symbol: row.symbol,
      baseSymbol: row.baseSymbol,
      closedTrades: row.closedTrades,
      winningTrades: row.winningTrades,
      losingTrades: row.losingTrades,
      realizedPnl: row.realizedPnl,
      turnover: row.turnover,
      commission: row.commission,
      longPnl: row.longPnl,
      shortPnl: row.shortPnl,
      winRatePct: row.closedTrades > 0 ? row.winningTrades / row.closedTrades * 100 : 0,
      capitalContributionPct: initialCapital > 0 ? row.realizedPnl / initialCapital * 100 : 0,
      turnoverReturnPct: row.turnover > 0 ? row.realizedPnl / row.turnover * 100 : 0,
    })
  }
  return result.sort((left, right) =>
    right.realizedPnl - left.realizedPnl
    || right.closedTrades - left.closedTrades
    || left.symbol.localeCompare(right.symbol),
  )
}

function ensureRow(rows: Map<string, MutableSymbolPerformance>, symbol: string) {
  let row = rows.get(symbol)
  if (!row) {
    row = {
      symbol,
      baseSymbol: symbol.split('-')[0] || symbol,
      closedTrades: 0,
      winningTrades: 0,
      losingTrades: 0,
      realizedPnl: 0,
      turnover: 0,
      commission: 0,
      longPnl: 0,
      shortPnl: 0,
    }
    rows.set(symbol, row)
  }
  return row
}

function isRealizedTrade(trade: BacktestTrade, action: string) {
  if (action === 'close') return true
  if (hasText(trade.exit_time)) return true
  if (isFiniteNumber(trade.exit_price) && trade.exit_price > 0) return true
  return Number.isFinite(trade.pnl) && trade.pnl !== 0
}

function tradeTurnover(trade: BacktestTrade) {
  const value = finiteNumber(trade.value)
  if (value > 0) return value
  const price = firstPositive(trade.exit_price, trade.price, trade.entry_price)
  const quantity = finiteNumber(trade.quantity)
  return price > 0 && quantity > 0 ? price * quantity : 0
}

function positionSide(trade: BacktestTrade, action: string) {
  const explicit = normalizeLower(trade.pos_side)
  if (explicit === 'long' || explicit === 'short') return explicit
  if (action === 'close') return trade.side === 'buy' ? 'short' : 'long'
  return trade.side === 'sell' ? 'short' : 'long'
}

function finiteNumber(value: number | null | undefined): number {
  return isFiniteNumber(value) ? value : 0
}

function firstPositive(
  first: number | null | undefined,
  second: number | null | undefined,
  third: number | null | undefined,
) {
  if (isFiniteNumber(first) && first > 0) return first
  if (isFiniteNumber(second) && second > 0) return second
  if (isFiniteNumber(third) && third > 0) return third
  return 0
}

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function normalizedSymbol(value: string | undefined, normalizedSymbols: Map<string | undefined, string>) {
  let symbol = normalizedSymbols.get(value)
  if (symbol === undefined) {
    symbol = normalizeUpper(value)
    normalizedSymbols.set(value, symbol)
  }
  return symbol
}

function normalizeLower(value: string) {
  if (value === 'open' || value === 'close' || value === 'long' || value === 'short') return value
  return value ? value.trim().toLowerCase() : ''
}

function normalizeUpper(value: string | undefined) {
  return value ? value.trim().toUpperCase() : ''
}

function hasText(value: string) {
  return value ? value.trim().length > 0 : false
}
