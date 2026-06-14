import { computed, ref } from 'vue'
import { cancelOrder as apiCancel } from '@/api/trading'
import { useSystemStore } from '@/stores/systemStore'
import type { Order } from '@/types'
import { describeError } from '@/utils/logger'

type TradingMode = 'simulated' | 'live'

interface PendingOrdersProps {
  orders: Order[]
  mode?: string
  modeLocked?: boolean
}

interface UsePendingOrdersOptions {
  onCancelled?: () => void
}

const STATE_LABELS: Record<string, string> = {
  live: '活跃',
  partially_filled: '部分成交',
  filled: '已成交',
  cancelled: '已撤销',
}

export function usePendingOrders(
  props: PendingOrdersProps,
  options: UsePendingOrdersOptions = {},
) {
  const systemStore = useSystemStore()
  const cancelling = ref(new Set<string>())
  const error = ref<string | null>(null)

  const pendingOrders = computed(() =>
    props.orders.filter(order => order.state === 'live' || order.state === 'partially_filled'),
  )

  const resolvedMode = computed(() => normalizeMode(props.mode) || systemStore.tradingMode)
  const modeLocked = computed(() => props.modeLocked === true)
  const modeLabel = computed(() => {
    const label = resolvedMode.value === 'live' ? '实盘' : '模拟盘'
    return modeLocked.value ? `查看：${label} LOCK` : `查看：${label}`
  })

  function stateLabel(state: string): string {
    return STATE_LABELS[state] || state
  }

  async function cancel(order: Order) {
    if (modeLocked.value) {
      error.value = '当前查看模式与系统默认交易模式不一致，已禁止撤单。请在“系统设置”切换默认模式后重试。'
      return
    }
    cancelling.value.add(order.ord_id)
    cancelling.value = new Set(cancelling.value)
    error.value = null
    try {
      if (!systemStore.statusLoaded) {
        await systemStore.loadConfig()
      }
      await apiCancel(
        order.ord_id,
        order.inst_id,
        resolvedMode.value,
      )
      options.onCancelled?.()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      cancelling.value.delete(order.ord_id)
      cancelling.value = new Set(cancelling.value)
    }
  }

  return {
    cancelling,
    error,
    pendingOrders,
    resolvedMode,
    modeLocked,
    modeLabel,
    stateLabel,
    cancel,
  }
}

function normalizeMode(value: unknown): TradingMode | '' {
  const mode = String(value || '').trim().toLowerCase()
  if (mode === 'live') return 'live'
  if (mode === 'simulated') return 'simulated'
  return ''
}
