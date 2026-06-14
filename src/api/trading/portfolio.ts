import { apiGet } from '../client'
import {
  arrayRecords,
  arrayValue,
} from '../normalize'
import {
  normalizeAccount,
  normalizePosition,
} from './normalize'
import { modeParams } from './shared'

export function fetchAccount(mode?: string) {
  return apiGet<unknown>('/api/trading/account', modeParams(mode)).then(normalizeAccount)
}

export function fetchPositions(mode?: string) {
  return apiGet<unknown>('/api/trading/positions', modeParams(mode))
    .then(data => arrayRecords(data).map(normalizePosition))
}

export function fetchSpotHoldings(mode?: string) {
  return apiGet<unknown>('/api/trading/spot-holdings', modeParams(mode))
    .then(arrayValue)
}
