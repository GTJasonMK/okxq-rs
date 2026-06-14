import type { Orderbook } from '@/types'

type OrderbookDisplayRow = {
  price: number
  size: number
  depthPct: number
}

export type OrderbookDisplaySide = {
  best: Orderbook['bids'][number] | null
  rows: OrderbookDisplayRow[]
}

export function mergeDepthOrderbook(realtime: Orderbook | null, snapshot: Orderbook | null): Orderbook | null {
  if (!realtime) return snapshot
  if (!snapshot || snapshot.inst_id !== realtime.inst_id) return realtime
  return {
    inst_id: realtime.inst_id,
    bids: mergeBookSide(realtime.bids, snapshot.bids, 'bid'),
    asks: mergeBookSide(realtime.asks, snapshot.asks, 'ask'),
    ts: realtime.ts || snapshot.ts,
  }
}

export function mergeBookSide(
  realtimeRows: Orderbook['bids'],
  snapshotRows: Orderbook['bids'],
  side: 'bid' | 'ask',
) {
  const sortedRealtime = sortedOrderbookSide(realtimeRows, side)
  if (sortedRealtime.length === 0) {
    return sortedOrderbookSide(snapshotRows, side)
  }

  const realtimeFarEdge = side === 'bid'
    ? Math.min(...sortedRealtime.map(row => row.price))
    : Math.max(...sortedRealtime.map(row => row.price))
  const snapshotDeepRows = snapshotRows.filter(row => (
    isValidOrderbookLevel(row) &&
    (side === 'bid' ? row.price < realtimeFarEdge : row.price > realtimeFarEdge)
  ))
  const merged = [...sortedRealtime, ...snapshotDeepRows]
  return sortedOrderbookSide(merged, side)
}

function isValidOrderbookLevel(row: { price: number; size: number }) {
  return Number.isFinite(row.price) && row.price > 0 && Number.isFinite(row.size) && row.size > 0
}

export function sortedOrderbookSide<T extends { price: number; size: number }>(
  rows: T[],
  side: 'bid' | 'ask',
): T[] {
  const validRows: T[] = []
  let sorted = true
  let previousPrice: number | null = null
  for (const row of rows) {
    if (!isValidOrderbookLevel(row)) continue
    if (previousPrice !== null) {
      const outOfOrder = side === 'bid'
        ? row.price > previousPrice
        : row.price < previousPrice
      if (outOfOrder) sorted = false
    }
    previousPrice = row.price
    validRows.push(row)
  }
  return sorted
    ? validRows
    : validRows.sort((a, b) => side === 'bid' ? b.price - a.price : a.price - b.price)
}

function topOrderbookSide<T extends { price: number; size: number }>(
  rows: T[],
  side: 'bid' | 'ask',
  limit: number,
): T[] {
  const normalizedLimit = Math.max(0, Math.floor(Number.isFinite(limit) ? limit : 0))
  if (normalizedLimit === 0) return []
  const selected: T[] = []
  for (const row of rows) {
    if (!isValidOrderbookLevel(row)) continue
    let inserted = false
    for (let index = 0; index < selected.length; index += 1) {
      if (isBetterOrderbookPrice(row.price, selected[index].price, side)) {
        selected.splice(index, 0, row)
        inserted = true
        break
      }
    }
    if (!inserted && selected.length < normalizedLimit) {
      selected.push(row)
    }
    if (selected.length > normalizedLimit) selected.pop()
  }
  return selected
}

export function orderbookDisplaySide(
  rows: Orderbook['bids'],
  side: 'bid' | 'ask',
  limit: number,
): OrderbookDisplaySide {
  const topRows = topOrderbookSide(rows, side, limit)
  const best = topRows[0] ?? null
  const displayRows = side === 'ask' ? topRows.slice().reverse() : topRows
  let maxSize = 1
  for (const row of displayRows) maxSize = Math.max(maxSize, row.size)
  return {
    best,
    rows: displayRows.map(row => ({
      price: row.price,
      size: row.size,
      depthPct: (row.size / maxSize) * 100,
    })),
  }
}

function isBetterOrderbookPrice(price: number, selectedPrice: number, side: 'bid' | 'ask') {
  return side === 'bid' ? price > selectedPrice : price < selectedPrice
}
