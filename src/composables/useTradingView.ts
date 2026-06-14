import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import * as api from '@/api/trading'
import { usePrivateTradingRealtime } from '@/composables/usePrivateTradingRealtime'
import { useSystemStore } from '@/stores/systemStore'
import { useTradingStore } from '@/stores/tradingStore'
import type { Position } from '@/types'
import { describeError } from '@/utils/logger'
import { settledErrorMessage } from '@/utils/settled'
import { ensureTradingStoreMode } from '@/utils/tradingStoreMode'

type TradingViewMode = 'simulated' | 'live'

const TRADING_REFRESH_LABELS = ['账户', '持仓', '挂单', '成交']

export function useTradingView() {
  const route = useRoute()
  const store = useTradingStore()
  const systemStore = useSystemStore()
  const loading = ref(false)
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)
  const closingPositionKeys = ref(new Set<string>())

  const viewMode = computed(() => normalizeRouteMode(route.query.mode) || systemStore.tradingMode)
  const {
    connected: privateRealtimeConnected,
    error: privateRealtimeError,
    connectedMode: privateRealtimeMode,
  } = usePrivateTradingRealtime(() => viewMode.value)
  const viewModeLabel = computed(() => viewMode.value === 'live' ? '实盘' : '模拟盘')
  const viewModeLocked = computed(
    () => systemStore.statusLoaded && viewMode.value !== systemStore.tradingMode,
  )
  let refreshSequence = 0

  async function loadAccountAndPositions() {
    const mode = viewMode.value
    const sequence = ++refreshSequence
    ensureTradingStoreMode(store, mode)
    const tasks = await Promise.allSettled([
      api.fetchAccount(mode),
      api.fetchPositions(mode),
      api.fetchOrders(mode),
      api.fetchFills(100, mode),
    ])
    if (sequence !== refreshSequence || mode !== viewMode.value) return
    if (tasks[0].status === 'fulfilled') store.account = tasks[0].value
    if (tasks[1].status === 'fulfilled') store.positions = tasks[1].value
    if (tasks[2].status === 'fulfilled') store.orders = tasks[2].value
    if (tasks[3].status === 'fulfilled') store.fills = tasks[3].value
    const refreshError = settledErrorMessage(tasks, TRADING_REFRESH_LABELS)
    if (refreshError) error.value = refreshError
  }

  async function refreshAll(clearMessage = true) {
    error.value = null
    if (clearMessage) message.value = null
    loading.value = true
    try {
      await loadAccountAndPositions()
    } finally {
      loading.value = false
    }
  }

  async function handleOrderSubmitted() {
    message.value = '订单已提交，等待 OKX 私有 WebSocket 回报'
    error.value = null
    void loadAccountAndPositions()
  }

  async function handleOrderCancelled() {
    message.value = '撤单请求已提交，等待 OKX 私有 WebSocket 回报'
    error.value = null
    void loadAccountAndPositions()
  }

  async function handleClosePosition(position: Position) {
    if (viewModeLocked.value) {
      error.value = `当前查看的是${viewModeLabel.value}数据，系统默认交易模式为${systemStore.tradingModeLabel}，已禁止平仓。`
      return
    }
    if (position.inst_type !== 'SWAP') {
      error.value = `${position.inst_id} 不是合约持仓，不能通过合约平仓入口处理。`
      return
    }
    if (!Number.isFinite(position.pos)) {
      error.value = `${position.inst_id} 持仓数量无效，已拒绝平仓。`
      return
    }
    const size = Math.abs(position.pos as number)
    if (size <= 0) {
      error.value = `${position.inst_id} 持仓数量无效，已拒绝平仓。`
      return
    }

    const key = positionCloseKey(position)
    if (closingPositionKeys.value.has(key)) return
    closingPositionKeys.value.add(key)
    closingPositionKeys.value = new Set(closingPositionKeys.value)
    error.value = null
    message.value = null
    try {
      if (!systemStore.statusLoaded) {
        await systemStore.loadConfig()
      }
      await api.placeOrder({
        inst_id: position.inst_id,
        inst_type: position.inst_type,
        td_mode: position.mgn_mode === 'isolated' ? 'isolated' : 'cross',
        side: position.pos_side === 'short' ? 'buy' : 'sell',
        ord_type: 'market',
        sz: size,
        pos_side: position.pos_side,
        reduce_only: true,
        mode: viewMode.value,
      })
      message.value = `${position.inst_id} ${position.pos_side === 'short' ? '平空' : '平多'}市价单已提交，等待 OKX 私有 WebSocket 回报`
      void loadAccountAndPositions()
    } catch (caught) {
      error.value = `平仓失败：${describeError(caught)}`
    } finally {
      closingPositionKeys.value.delete(key)
      closingPositionKeys.value = new Set(closingPositionKeys.value)
    }
  }

  onMounted(async () => {
    if (!systemStore.statusLoaded) {
      await systemStore.loadConfig()
    }
    await refreshAll()
  })

  watch(viewMode, () => {
    void refreshAll()
  })

  return {
    store,
    systemStore,
    loading,
    error,
    message,
    privateRealtimeConnected,
    privateRealtimeError,
    privateRealtimeMode,
    closingPositionKeys,
    viewMode,
    viewModeLabel,
    viewModeLocked,
    refreshAll,
    handleOrderSubmitted,
    handleOrderCancelled,
    handleClosePosition,
  }
}

function positionCloseKey(position: Position): string {
  return `${position.inst_id}:${position.pos_side}`
}

function normalizeRouteMode(value: unknown): TradingViewMode | '' {
  const mode = String(Array.isArray(value) ? value[0] : value || '').trim().toLowerCase()
  if (mode === 'live') return 'live'
  if (mode === 'simulated') return 'simulated'
  return ''
}
