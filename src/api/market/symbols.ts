import { apiGet } from '../client'
import { arrayValue } from '../normalize'
import {
  normalizeMarketSymbol,
} from '../marketNormalize'

export function fetchSymbols() {
  return apiGet<unknown>('/api/market/symbols')
    .then(data => arrayValue(data).map(normalizeMarketSymbol))
}
