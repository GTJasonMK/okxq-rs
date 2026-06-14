import { computed, onMounted, ref, watch } from 'vue'
import * as api from '@/api/trading'
import { usePrivateTradingRealtime } from '@/composables/usePrivateTradingRealtime'
import { useSystemStore } from '@/stores/systemStore'
import { useTradingStore } from '@/stores/tradingStore'
import { settledErrorMessage } from '@/utils/settled'
import { ensureTradingStoreMode } from '@/utils/tradingStoreMode'

const LOAD_LABELS = ['账户', '持仓']

export function useDashboardView() {
  const trading = useTradingStore()
  const systemStore = useSystemStore()
  const viewMode = computed(() => systemStore.tradingMode)
  const viewModeLabel = computed(() => viewMode.value === 'live' ? '实盘' : '模拟盘')
  const {
    connected: privateRealtimeConnected,
    error: privateRealtimeError,
    connectedMode: privateRealtimeMode,
  } = usePrivateTradingRealtime(() => viewMode.value)
  const error = ref<string | null>(null)
  let loadSequence = 0

  const unrealizedPnl = computed(() => sumKnown(trading.positions.map(position => position.upl)))

  const totalPnl = computed(() => unrealizedPnl.value)

  async function loadData() {
    const mode = viewMode.value
    const sequence = ++loadSequence
    ensureTradingStoreMode(trading, mode)
    error.value = null
    const tasks = await Promise.allSettled([api.fetchAccount(mode), api.fetchPositions(mode)])
    if (sequence !== loadSequence || mode !== viewMode.value) return
    if (tasks[0].status === 'fulfilled') trading.account = tasks[0].value
    if (tasks[1].status === 'fulfilled') trading.positions = tasks[1].value
    error.value = settledErrorMessage(tasks, LOAD_LABELS)
  }

  onMounted(() => {
    if (!systemStore.statusLoaded) {
      void systemStore.loadConfig().then(loadData)
      return
    }
    void loadData()
  })

  watch(viewMode, () => {
    void loadData()
  })

  return {
    trading,
    error,
    viewMode,
    viewModeLabel,
    privateRealtimeConnected,
    privateRealtimeError,
    privateRealtimeMode,
    unrealizedPnl,
    totalPnl,
  }
}

function sumKnown(values: Array<number | null>) {
  let found = false
  let total = 0
  for (const value of values) {
    if (!Number.isFinite(value)) continue
    found = true
    total += value as number
  }
  return found ? total : null
}
