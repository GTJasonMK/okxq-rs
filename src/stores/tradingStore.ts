import { defineStore } from 'pinia'
import { ref } from 'vue'
import type {
  AccountInfo,
  Position,
  Order,
  Fill,
  CostBasis,
  TradePerformance,
  TradingMode,
} from '@/types'

export const useTradingStore = defineStore('trading', () => {
  const mode = ref<TradingMode>('simulated')
  const account = ref<AccountInfo | null>(null)
  const positions = ref<Position[]>([])
  const orders = ref<Order[]>([])
  const fills = ref<Fill[]>([])
  const costBasis = ref<CostBasis[]>([])
  const performance = ref<TradePerformance[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  function setMode(nextMode: TradingMode) {
    if (mode.value === nextMode) return
    mode.value = nextMode
    account.value = null
    positions.value = []
    orders.value = []
    fills.value = []
    costBasis.value = []
    performance.value = []
    error.value = null
  }

  return {
    mode,
    account,
    positions,
    orders,
    fills,
    costBasis,
    performance,
    loading,
    error,
    setMode,
  }
})
