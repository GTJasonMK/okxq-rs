import type { useTradingStore } from '@/stores/tradingStore'
import type { TradingMode } from '@/types'

type TradingStore = ReturnType<typeof useTradingStore>
type TradingStoreWithOptionalModeAction = Omit<TradingStore, 'setMode'> & {
  setMode?: unknown
}

export function ensureTradingStoreMode(store: TradingStore, nextMode: TradingMode) {
  const storeWithOptionalAction = store as TradingStoreWithOptionalModeAction
  const setMode = storeWithOptionalAction.setMode
  if (typeof setMode === 'function') {
    ;(setMode as (mode: TradingMode) => void)(nextMode)
    return
  }

  if (storeWithOptionalAction.mode === nextMode) return
  storeWithOptionalAction.mode = nextMode
  storeWithOptionalAction.account = null
  storeWithOptionalAction.positions = []
  storeWithOptionalAction.orders = []
  storeWithOptionalAction.fills = []
  storeWithOptionalAction.costBasis = []
  storeWithOptionalAction.performance = []
  storeWithOptionalAction.error = null
}
