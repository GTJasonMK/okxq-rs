import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as backtestApi from '@/api/backtest'
import { useBacktestStore } from '@/stores/backtestStore'
import type { BacktestProgress, BacktestResult } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import { describeError } from '@/utils/logger'
import { clampCandleRangeDaysForTimeframe, DEFAULT_CANDLE_RANGE_DAYS } from '@/utils/marketView'
import { backtestTradesToMarkers } from '@/utils/strategyTriggers'

export function useBacktestView() {
  const store = useBacktestStore()

  const strategyId = ref('')
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)
  const deletingResultId = ref<string | null>(null)
  const runProgress = ref<BacktestProgress | null>(null)
  let progressPollTimer: ReturnType<typeof setInterval> | null = null
  let activeProgressId = ''
  const backtestMarkers = computed(() =>
    backtestTradesToMarkers(store.activeResult?.trades ?? [])
  )
  const backtestChartRangeDays = computed<CandleRangeDays>(() => {
    const result = store.activeResult
    if (!result) return DEFAULT_CANDLE_RANGE_DAYS
    return clampCandleRangeDaysForTimeframe(result.days || DEFAULT_CANDLE_RANGE_DAYS, result.timeframe)
  })
  async function loadStrategies() {
    try {
      store.strategies = await backtestApi.fetchStrategies()
      if (!strategyId.value) strategyId.value = store.strategies[0]?.id ?? ''
    } catch (e) {
      error.value = `策略列表: ${describeError(e)}`
    }
  }

  async function loadHistory() {
    try {
      store.history = await backtestApi.fetchBacktestHistory()
    } catch (e) {
      error.value = `历史回测: ${describeError(e)}`
    }
  }

  async function run(payload: Record<string, unknown> = {}) {
    await runBacktestRequest(strategyId.value, payload)
  }

  async function runBacktestRequest(targetStrategyId: string, payload: Record<string, unknown>) {
    if (!targetStrategyId || store.running) return
    error.value = null
    message.value = null
    store.running = true
    const progressId = createBacktestProgressId(targetStrategyId)
    startProgressPolling(progressId, targetStrategyId)
    try {
      const result = await backtestApi.runBacktest(targetStrategyId, {
        ...payload,
        progress_id: progressId,
      })
      await loadHistory()
      store.activeResult = result || store.history[0] || null
      finishProgress('completed', '回测完成')
      message.value = resultMessage(store.activeResult, targetStrategyId)
    } catch (e) {
      finishProgress('failed', `回测失败: ${describeError(e)}`)
      error.value = describeError(e)
    } finally {
      store.running = false
      stopProgressPolling()
    }
  }

  async function selectResult(result: BacktestResult) {
    error.value = null
    try {
      store.activeResult = await backtestApi.fetchBacktestDetail(result.result_id)
    } catch (e) {
      error.value = `回测详情加载失败，已显示摘要: ${describeError(e)}`
      store.activeResult = result
    }
  }

  async function deleteResult(result: BacktestResult) {
    if (deletingResultId.value) return
    if (!result.result_id) {
      error.value = '删除回测记录失败: 缺少回测记录 ID'
      return
    }

    error.value = null
    message.value = null
    deletingResultId.value = result.result_id
    try {
      await backtestApi.deleteBacktestResult(result.result_id)
      store.history = store.history.filter(item => item.result_id !== result.result_id)
      if (store.activeResult?.result_id === result.result_id) {
        store.activeResult = null
      }
      message.value = '回测记录已删除'
    } catch (e) {
      error.value = `删除回测记录失败: ${describeError(e)}`
    } finally {
      deletingResultId.value = null
    }
  }

  function startProgressPolling(progressId: string, activeStrategyId: string) {
    stopProgressPolling()
    activeProgressId = progressId
    runProgress.value = {
      run_id: progressId,
      strategy_id: activeStrategyId,
      status: 'running',
      stage: 'prepare',
      message: '准备运行策略',
      progress: 1,
      processed_candles: 0,
      total_candles: 0,
      started_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }
    void pollBacktestProgress(progressId)
    progressPollTimer = setInterval(() => {
      void pollBacktestProgress(progressId)
    }, 500)
  }

  function stopProgressPolling() {
    if (progressPollTimer) {
      clearInterval(progressPollTimer)
      progressPollTimer = null
    }
    activeProgressId = ''
  }

  async function pollBacktestProgress(progressId: string) {
    try {
      const progress = await backtestApi.fetchBacktestProgress(progressId)
      if (activeProgressId !== progressId) return
      if (store.running && progress.status === 'idle') return
      runProgress.value = progress
    } catch (e) {
      if (activeProgressId !== progressId || !runProgress.value) return
      runProgress.value = {
        ...runProgress.value,
        message: `进度读取失败，等待回测结果返回: ${describeError(e)}`,
        updated_at: new Date().toISOString(),
      }
    }
  }

  function finishProgress(status: BacktestProgress['status'], progressMessage: string) {
    if (!runProgress.value) return
    runProgress.value = {
      ...runProgress.value,
      status,
      stage: status === 'failed' ? 'failed' : 'complete',
      message: progressMessage,
      progress: 100,
      updated_at: new Date().toISOString(),
    }
  }

  onMounted(() => {
    void loadStrategies()
    void loadHistory()
  })

  onUnmounted(() => {
    stopProgressPolling()
  })

  watch(strategyId, () => {
    message.value = null
  })

  return {
    store,
    strategyId,
    error,
    message,
    runProgress,
    backtestMarkers,
    backtestChartRangeDays,
    deletingResultId,
    run,
    selectResult,
    deleteResult,
  }
}

function createBacktestProgressId(strategyId: string) {
  const safeStrategyId = strategyId.replace(/[^a-zA-Z0-9_-]/g, '_') || 'strategy'
  const suffix = Math.random().toString(36).slice(2, 8)
  return `${safeStrategyId}_${Date.now()}_${suffix}`
}

function resultMessage(result: BacktestResult | null, strategyId: string) {
  if (!result) return '回测运行完成'
  return `${result.strategy_name || strategyId} 回测运行完成`
}
