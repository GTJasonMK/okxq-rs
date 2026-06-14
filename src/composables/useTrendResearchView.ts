import { onMounted, ref } from 'vue'
import * as marketApi from '@/api/market'
import * as api from '@/api/research'
import type { InstType, Timeframe } from '@/types'
import { describeError } from '@/utils/logger'

type TrendConfig = {
  symbol: string
  inst_type: Extract<InstType, 'SPOT' | 'SWAP'>
  timeframe: Timeframe
  bar_count: number
}

type TrendFactorValue = {
  name: string
  value: number
}

const DEFAULT_BAR_COUNT = 500

export function useTrendResearchView() {
  const factors = ref<TrendFactorValue[]>([])
  const computing = ref(false)
  const config = ref<TrendConfig>({
    symbol: '',
    inst_type: 'SWAP',
    timeframe: '1H',
    bar_count: DEFAULT_BAR_COUNT,
  })
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)

  async function applyWatchScope(raw?: Record<string, unknown>) {
    const rawSymbol = String(raw?.symbol || config.value.symbol || '')
    const rawInstType = String(raw?.inst_type || config.value.inst_type || '')
    const scope = await marketApi.fetchDefaultWatchScope({
      symbol: rawSymbol,
      inst_type: rawInstType,
    })
    if (!scope) throw new Error('请先在数据中心添加关注币种并启用数据目标')
    config.value = {
      symbol: scope.symbol,
      inst_type: scope.inst_type,
      timeframe: (raw?.timeframe as Timeframe) || config.value.timeframe || '1H',
      bar_count: normalizeBarCount(raw?.bar_count, config.value.bar_count),
    }
  }

  async function loadFactors(limit = 20) {
    factors.value = await api.fetchTrendFactors(config.value.symbol, limit, {
      inst_type: config.value.inst_type,
      timeframe: config.value.timeframe,
    })
  }

  async function computeFactors() {
    computing.value = true
    error.value = null
    message.value = null
    try {
      await applyWatchScope(config.value)
      factors.value = await api.computeFactors(config.value.symbol, config.value.bar_count, {
        inst_type: config.value.inst_type,
        timeframe: config.value.timeframe,
      })
      message.value = `已计算 ${factors.value.length} 个因子`
    } catch (e) {
      error.value = describeError(e)
    } finally {
      computing.value = false
    }
  }

  async function saveConfig(nextConfig: Record<string, unknown>) {
    error.value = null
    message.value = null
    try {
      await applyWatchScope(nextConfig)
      await api.updateTrendConfig(config.value)
      message.value = '趋势研究配置已保存'
      await loadFactors()
    } catch (e) {
      error.value = describeError(e)
    }
  }

  onMounted(async () => {
    try {
      const data = await api.fetchTrendConfig() as Record<string, unknown>
      await applyWatchScope(data)
      await loadFactors()
    } catch (e) {
      error.value = describeError(e)
    }
  })

  return {
    factors,
    computing,
    config,
    error,
    message,
    computeFactors,
    saveConfig,
  }
}

function normalizeBarCount(value: unknown, fallback = DEFAULT_BAR_COUNT) {
  const candidate = typeof value === 'number' && Number.isFinite(value) ? value : fallback
  const rounded = Math.round(candidate)
  return Number.isFinite(rounded) && rounded > 0 ? rounded : DEFAULT_BAR_COUNT
}
