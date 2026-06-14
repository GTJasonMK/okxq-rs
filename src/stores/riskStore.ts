import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useRiskStore = defineStore('risk', () => {
  const snapshots = ref<unknown[]>([])
  const varMetrics = ref<unknown | null>(null)
  const drawdown = ref<unknown | null>(null)
  const rolling = ref<unknown | null>(null)
  const loading = ref(false)
  return { snapshots, varMetrics, drawdown, rolling, loading }
})
