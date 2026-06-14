import type { LiveOrder } from '@/types'
import {
  finiteOrZero,
  validPositiveTimestamp,
} from '@/utils/liveStrategyCore/sort'

export function compareOrdersByLatest(left: LiveOrder, right: LiveOrder): number {
  return orderTimestamp(right) - orderTimestamp(left)
    || finiteOrZero(right.created_at) - finiteOrZero(left.created_at)
    || finiteOrZero(right.id) - finiteOrZero(left.id)
}

export function orderTimestamp(order: LiveOrder): number {
  if (validPositiveTimestamp(order.timestamp)) return order.timestamp
  if (validPositiveTimestamp(order.created_at)) return order.created_at
  return 0
}
