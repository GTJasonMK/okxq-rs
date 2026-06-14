import type { ComputedRef, Ref } from 'vue'
import * as api from '@/api/live'
import type { LiveStrategyStatus } from '@/types'
import { describeError } from '@/utils/logger'
import type { LiveActionPhase } from '@/utils/liveStrategyControlView'

type LiveStrategyActionSystemStore = {
  statusLoaded: boolean
  loadConfig: () => Promise<unknown> | unknown
}

type LiveStrategyActionForm = {
  strategy_id: string
  symbol: string
  inst_type: string
  timeframe: string
  risk_timeframe: string
  initial_capital: number
  position_size: number
  stop_loss: number
  take_profit: number
  check_interval: number
  params: Record<string, unknown>
}

type LiveStrategyActionDeps = {
  systemStore: LiveStrategyActionSystemStore
  status: Ref<LiveStrategyStatus | null>
  error: Ref<string | null>
  message: Ref<string | null>
  actionLoading: Ref<boolean>
  actionPhase: Ref<LiveActionPhase>
  form: LiveStrategyActionForm
  launchMode: ComputedRef<string>
  startDisabledReason: ComputedRef<string>
  stopDisabledReason: ComputedRef<string>
  enforceSupportedTimeframe: () => void
  enforceSupportedSymbol: () => void
  loadData: () => Promise<void>
}

export function useLiveStrategyActions({
  systemStore,
  status,
  error,
  message,
  actionLoading,
  actionPhase,
  form,
  launchMode,
  startDisabledReason,
  stopDisabledReason,
  enforceSupportedTimeframe,
  enforceSupportedSymbol,
  loadData,
}: LiveStrategyActionDeps) {
  async function startStrategy(overrides: Record<string, unknown> = {}) {
    if (startDisabledReason.value) {
      error.value = startDisabledReason.value
      return
    }
    if (!systemStore.statusLoaded) {
      await systemStore.loadConfig()
    }
    if (startDisabledReason.value) {
      error.value = startDisabledReason.value
      return
    }
    const mode = launchMode.value
    enforceSupportedTimeframe()
    enforceSupportedSymbol()
    error.value = null
    message.value = null
    actionLoading.value = true
    actionPhase.value = 'starting'
    try {
      status.value = await api.startLiveStrategy({
        strategy_id: form.strategy_id,
        symbol: form.symbol,
        inst_type: form.inst_type,
        timeframe: form.timeframe,
        risk_timeframe: form.risk_timeframe,
        initial_capital: form.initial_capital,
        position_size: form.position_size,
        stop_loss: form.stop_loss,
        take_profit: form.take_profit,
        check_interval: form.check_interval,
        params: form.params,
        mode,
        ...overrides,
      })
      message.value = `策略已启动（${mode === 'live' ? '实盘' : '模拟盘'}）`
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      actionLoading.value = false
      actionPhase.value = 'idle'
    }
  }

  async function stopStrategy() {
    if (stopDisabledReason.value) {
      error.value = stopDisabledReason.value
      return
    }
    error.value = null
    message.value = null
    actionLoading.value = true
    actionPhase.value = 'stopping'
    try {
      status.value = await api.stopLiveStrategy()
      message.value = '策略已停止'
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      actionLoading.value = false
      actionPhase.value = 'idle'
    }
  }

  return {
    startStrategy,
    stopStrategy,
  }
}
