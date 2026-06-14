import { onMounted, ref } from 'vue'
import * as api from '@/api/risk'
import { useRiskStore } from '@/stores/riskStore'
import { describeError } from '@/utils/logger'

export function useRiskView() {
  const store = useRiskStore()
  const error = ref<string | null>(null)

  async function loadData() {
    store.loading = true
    error.value = null
    try {
      const overview = await api.fetchOverview()
      store.snapshots = overview.snapshots
      store.varMetrics = overview.metrics
      store.drawdown = overview.drawdown
      store.rolling = overview.rolling
    } catch (reason) {
      error.value = describeError(reason)
    } finally {
      store.loading = false
    }
  }

  onMounted(() => {
    void loadData()
  })

  return { store, error, loadData }
}
