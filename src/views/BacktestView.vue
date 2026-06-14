<template>
  <div class="view-backtest">
    <div class="vb-header">
      <h2 class="vb-title">回测</h2>
      <div class="vb-config">
        <button class="run-btn" @click="openRunParams" :disabled="store.running || !strategyId">
          {{ store.running ? '运行中...' : '运行策略' }}
        </button>
      </div>
    </div>

    <div v-if="message" class="vb-message">{{ message }}</div>
    <div v-if="error" class="vb-error">{{ error }}</div>
    <BacktestRunProgress :progress="runProgress" />

    <div class="vb-grid">
      <aside class="vb-sidebar">
        <StrategySelector
          :model-value="strategyId"
          :strategies="store.strategies"
          @update:strategy-id="strategyId = $event"
        />

        <BacktestHistoryList
          :active-result-id="store.activeResult?.result_id"
          :deleting-result-id="deletingResultId"
          :history="store.history"
          @delete="deleteResult"
          @select="selectResult"
        />
      </aside>

      <main class="vb-main">
        <template v-if="store.activeResult">
          <section class="vb-chart-panel">
            <div class="vb-tabs vb-chart-tabs">
              <button
                class="vb-tab"
                :class="{ active: activeChartPanel === 'equity' }"
                @click="activeChartPanel = 'equity'"
              >
                余额K线
              </button>
              <button
                class="vb-tab"
                :class="{ active: activeChartPanel === 'symbols' }"
                @click="activeChartPanel = 'symbols'"
              >
                币种收益
                <span class="vb-tab-count">{{ symbolPerformanceRows.length }}</span>
              </button>
              <button
                class="vb-tab"
                :class="{ active: activeChartPanel === 'orders' }"
                @click="activeChartPanel = 'orders'"
              >
                订单明细
                <span class="vb-tab-count">{{ activeOrderCount }}</span>
              </button>
              <ThemeSelect
                v-if="activeChartPanel === 'equity'"
                class="equity-bucket"
                :model-value="equityTimeframe"
                :options="equityTimeframeOptions"
                size="sm"
                @update:model-value="equityTimeframe = $event as Timeframe"
              />
            </div>
            <div class="vb-chart-content" :class="`chart-${activeChartPanel}`">
              <EquityCandleChart
                v-if="activeChartPanel === 'equity'"
                title="账户余额"
                :candles="equityCandles"
                :snapshots="equitySnapshots"
                :timeframe="equityTimeframe"
                :trades="store.activeResult.trades || []"
              />
              <BacktestSymbolPerformance
                v-else-if="activeChartPanel === 'symbols'"
                :rows="symbolPerformanceRows"
                :displayed-events="store.activeResult.trades?.length || 0"
                :total-events="store.activeResult.trade_events_total"
                :truncated="store.activeResult.trades_truncated"
              />
              <LiveOrderTable
                v-else
                :orders="store.activeResult.orders || []"
                :show-charts="false"
              />
            </div>
          </section>
          <section class="vb-data-panel">
            <div class="vb-tabs vb-data-tabs">
              <button
                class="vb-tab"
                :class="{ active: activeDataPanel === 'overview' }"
                @click="activeDataPanel = 'overview'"
              >
                回测概览
              </button>
              <button
                class="vb-tab"
                :class="{ active: activeDataPanel === 'orderDistribution' }"
                @click="activeDataPanel = 'orderDistribution'"
              >
                订单分布
                <span class="vb-tab-count">{{ activeOrderCount }}</span>
              </button>
            </div>
            <div class="vb-data-content" :class="`data-${activeDataPanel}`">
              <BacktestResultCard
                v-if="activeDataPanel === 'overview'"
                :result="store.activeResult"
                :running="store.running"
              />
              <LiveOrderTable
                v-else
                :orders="store.activeResult.orders || []"
                :show-table="false"
              />
            </div>
          </section>
        </template>
        <div v-else class="vb-empty">
          <p>选择策略并运行，或从左侧选择历史结果</p>
        </div>
      </main>
    </div>
    <BacktestRunParamModal
      v-if="showRunParams"
      :running="store.running"
      :strategy="selectedStrategy"
      :strategy-id="strategyId"
      @close="showRunParams = false"
      @submit="submitRunParams"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useBacktestView } from '@/composables/useBacktestView'
import BacktestHistoryList from '@/components/backtest/BacktestHistoryList.vue'
import StrategySelector from '@/components/backtest/StrategySelector.vue'
import BacktestResultCard from '@/components/backtest/BacktestResultCard.vue'
import BacktestRunParamModal from '@/components/backtest/BacktestRunParamModal.vue'
import BacktestRunProgress from '@/components/backtest/BacktestRunProgress.vue'
import BacktestSymbolPerformance from '@/components/backtest/BacktestSymbolPerformance.vue'
import LiveOrderTable from '@/components/live/LiveOrderTable.vue'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import EquityCandleChart from '@/components/strategy/EquityCandleChart.vue'
import type { BacktestEquitySnapshot, Timeframe } from '@/types'
import { buildBacktestSymbolPerformance } from '@/utils/backtestSymbolPerformance'
import {
  buildEquityCandles,
  sortedEquitySnapshots,
} from '@/utils/strategyExecution'
import { normalizeTimeframe, VALID_MARKET_TIMEFRAMES } from '@/utils/marketView'

defineOptions({ name: 'BacktestView' })

type ChartPanel = 'equity' | 'symbols' | 'orders'
type DataPanel = 'overview' | 'orderDistribution'

const DEFAULT_EQUITY_TIMEFRAME: Timeframe = '15m'
const COMMON_EQUITY_TIMEFRAMES: Timeframe[] = ['15m', '1H', '4H', '1D']

const activeChartPanel = ref<ChartPanel>('equity')
const activeDataPanel = ref<DataPanel>('overview')
const equityTimeframe = ref<Timeframe>(DEFAULT_EQUITY_TIMEFRAME)
const showRunParams = ref(false)
const {
  store,
  strategyId,
  error,
  message,
  runProgress,
  deletingResultId,
  run,
  selectResult,
  deleteResult,
} = useBacktestView()

const activeResult = computed(() => store.activeResult)
const selectedStrategy = computed(() => store.strategies.find(item => item.id === strategyId.value) ?? null)
const activeResultTimeframe = computed(() => normalizedResultTimeframe(activeResult.value?.timeframe))
const equityTimeframeOptions = computed(() => {
  const seen = new Set<Timeframe>()
  return [activeResultTimeframe.value, ...COMMON_EQUITY_TIMEFRAMES]
    .filter((timeframe): timeframe is Timeframe => {
      if (seen.has(timeframe)) return false
      seen.add(timeframe)
      return true
    })
    .sort((left, right) => timeframeOrder(left) - timeframeOrder(right))
    .map(timeframe => ({ value: timeframe, label: timeframe }))
})
const equitySnapshots = computed<BacktestEquitySnapshot[]>(() => {
  const current = activeResult.value
  if (!current) return []
  if (current.equity_snapshots?.length) return sortedEquitySnapshots(current.equity_snapshots)
  return sortedEquitySnapshots(current.equity_curve.map(point => ({
    time: point.time,
    equity: point.equity,
    cash: point.equity,
    position_value: 0,
    position_notional: 0,
    unrealized_pnl: 0,
    position_side: 'flat',
    leverage: 1,
  })))
})
const equityCandles = computed(() => buildEquityCandles(equitySnapshots.value, equityTimeframe.value))
const activeOrderCount = computed(() => store.activeResult?.orders?.length ?? 0)
const symbolPerformanceRows = computed(() => {
  const result = activeResult.value
  if (!result) return []
  return buildBacktestSymbolPerformance(result.trades ?? [], result.initial_capital)
})

watch(activeResult, () => {
  activeChartPanel.value = 'equity'
  activeDataPanel.value = 'overview'
  equityTimeframe.value = activeResultTimeframe.value
}, { immediate: true })

function normalizedResultTimeframe(value: unknown): Timeframe {
  return normalizeTimeframe(value) || DEFAULT_EQUITY_TIMEFRAME
}

function timeframeOrder(value: Timeframe): number {
  const index = VALID_MARKET_TIMEFRAMES.indexOf(value)
  return index >= 0 ? index : Number.MAX_SAFE_INTEGER
}

function openRunParams() {
  if (store.running || !strategyId.value) return
  showRunParams.value = true
}

async function submitRunParams(payload: Record<string, unknown>) {
  showRunParams.value = false
  await run(payload)
}
</script>

<style scoped>
.view-backtest {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  min-height: 0;
  gap: 8px;
}
.vb-header { display: flex; flex: 0 0 auto; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; }
.vb-title { font-size: 16px; font-weight: 600; margin: 0; }
.vb-message,
.vb-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vb-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vb-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vb-config { display: flex; min-width: 0; gap: 6px; align-items: center; flex-wrap: wrap; }
.run-btn {
  padding: 5px 14px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.run-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.vb-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
  min-height: 0;
}
.vb-sidebar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vb-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vb-chart-panel,
.vb-data-panel {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  min-height: 0;
}
.vb-chart-panel {
  flex: 1 1 58%;
  min-height: 360px;
}
.vb-data-panel {
  flex: 0 1 38%;
  min-height: 240px;
}
.vb-tabs {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}
.vb-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 7px 12px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.vb-tab:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}
.vb-tab.active {
  border-color: rgba(41, 98, 255, 0.45);
  background: rgba(41, 98, 255, 0.16);
  color: var(--color-text-primary);
}
.vb-tab-count {
  min-width: 20px;
  padding: 1px 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.4;
  text-align: center;
}
.vb-tab.active .vb-tab-count {
  color: var(--color-text-primary);
}
.equity-bucket {
  width: 96px;
  margin-left: auto;
}
.vb-chart-content {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 320px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--color-bg-secondary);
}
.vb-data-content {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vb-data-content.data-overview {
  overflow: auto;
}
.vb-chart-content :deep(.equity-candle-chart),
.vb-chart-content :deep(.symbol-performance),
.vb-chart-content :deep(.lo-table) {
  height: 100%;
  border: none;
  border-radius: 0;
}
.vb-data-content :deep(.trade-list),
.vb-data-content :deep(.symbol-performance),
.vb-data-content :deep(.lo-table) {
  height: 100%;
  border: none;
  border-radius: 0;
}
.vb-chart-content :deep(.lo-table),
.vb-data-content :deep(.lo-table) {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.vb-chart-content :deep(.lo-wrap),
.vb-data-content :deep(.lo-wrap) {
  flex: 1 1 auto;
  max-height: none;
  min-height: 0;
}
.vb-chart-content :deep(.symbol-performance .sp-table-wrap) {
  height: calc(100% - 91px);
  max-height: none;
}
.vb-data-content :deep(.symbol-performance .sp-table-wrap) {
  height: calc(100% - 42px);
  max-height: none;
}
.vb-data-content :deep(.symbol-performance .sp-table-wrap) {
  height: calc(100% - 121px);
}
.vb-sidebar :deep(.vb-history) {
  flex: 1 1 auto;
  min-height: 0;
}
.vb-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-tertiary);
  font-size: 14px;
}
@media (max-width: 1100px) {
  .vb-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .vb-sidebar {
    max-height: 42vh;
  }
}

@media (max-width: 720px) {
  .run-btn {
    flex: 1 1 120px;
    width: auto;
  }
}
</style>
