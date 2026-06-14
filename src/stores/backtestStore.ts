import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { StrategyMeta, BacktestResult } from '@/types'

export const useBacktestStore = defineStore('backtest', () => {
  const strategies = ref<StrategyMeta[]>([])
  const history = ref<BacktestResult[]>([])
  const activeResult = ref<BacktestResult | null>(null)
  const running = ref(false)
  const loading = ref(false)

  return { strategies, history, activeResult, running, loading }
})
