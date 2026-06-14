<template>
  <div class="view-live">
    <div class="vl-topbar">
      <h2 class="vl-title">实盘策略</h2>
      <div class="vl-status-pill" :class="{ running: status?.running }">
        <span>策略运行</span>
        <strong>{{ status?.running ? '运行中' : '未运行' }}</strong>
      </div>
    </div>
    <div v-if="message" class="vl-message">{{ message }}</div>
    <div v-if="error" class="vl-error">{{ error }}</div>
    <div v-if="runtimeRefreshNotice" class="vl-runtime-warning">{{ runtimeRefreshNotice }}</div>
    <div class="vl-grid">
      <aside class="vl-sidebar">
        <LiveStrategyControlPanel
          :action-loading="actionLoading"
          :control-mode="controlMode"
          :control-mode-label="controlModeLabel"
          :detail-data-scope="detailDataScope"
          :form="form"
          :form-locked="formLocked"
          :launch-readiness="launchReadiness"
          :risk-scope-note="riskScopeNote"
          :start-button-text="startButtonText"
          :start-disabled-reason="startDisabledReason"
          :status="status"
          :stop-button-text="stopButtonText"
          :stop-disabled-reason="stopDisabledReason"
          :strategy-options="strategyOptions"
          @open-run-params="openRunParamModal"
          @stop="stopStrategy"
          @update-initial-capital="setInitialCapital"
          @update-strategy-id="setStrategyId"
        />
        <LiveExecutionLogPanel
          :logs="executionLogs"
          :error="executionLogRefreshError || ''"
        />
      </aside>
      <main class="vl-main">
        <section class="vl-focus-panel">
          <div class="vl-tabs vl-focus-tabs">
            <button
              type="button"
              class="vl-tab"
              :class="{ active: focusPanel === 'equity-chart' }"
              @click="focusPanel = 'equity-chart'"
            >
              余额K线
            </button>
	            <button
	              type="button"
	              class="vl-tab"
	              :class="{ active: focusPanel === 'decision' }"
	              @click="focusPanel = 'decision'"
	            >
	              决策
	            </button>
            <button
              type="button"
              class="vl-tab"
              :class="{ active: focusPanel === 'equity-details' }"
              @click="focusPanel = 'equity-details'"
            >
              权益明细
              <span class="vl-tab-count">{{ liveEquitySnapshots.length }}</span>
            </button>
            <button
              type="button"
              class="vl-tab"
              :class="{ active: focusPanel === 'positions' }"
              @click="focusPanel = 'positions'"
            >
              仓位
              <span class="vl-tab-count">{{ positions.length }}</span>
            </button>
            <button
              type="button"
              class="vl-tab"
              :class="{ active: focusPanel === 'planned-exits' }"
              @click="focusPanel = 'planned-exits'"
            >
              退出计划
              <span class="vl-tab-count">{{ scopedExecutionPlans.length }}</span>
            </button>
	            <ThemeSelect
              v-if="focusPanel === 'equity-chart'"
              class="vl-equity-bucket"
              :model-value="triggerTimeframe"
              :options="triggerTimeframeOptions"
              placeholder="周期"
              size="sm"
              @update:model-value="setTriggerTimeframe"
            />
            <ThemeSelect
              v-if="focusPanel !== 'equity-chart' && focusPanel !== 'positions' && focusPanel !== 'planned-exits'"
              class="vl-trigger-select"
              :model-value="selectedTriggerSymbol"
              :options="triggerSymbolOptions"
              placeholder="选择品种"
              size="sm"
              @update:model-value="setTriggerSymbol"
            />
          </div>
          <div class="vl-focus-content" :class="`focus-${focusPanel}`">
            <LiveStrategyEquityChartPanel
              v-if="focusPanel === 'equity-chart'"
              :candles="liveEquityCandles"
              :snapshots="liveEquitySnapshots"
              :timeframe="triggerTimeframe"
              :trades="liveEquityTrades"
            />
	            <LiveDecisionDiagnosticsPanel
	              v-else-if="focusPanel === 'decision'"
	              :diagnostics="currentDecisionDiagnostics"
	              :scope-text="decisionDiagnosticsScopeText"
	              :loading="decisionDiagnosticsLoading"
	              :refresh-source="decisionDiagnosticsRefreshSource || ''"
	              :error="decisionDiagnosticsError || ''"
	              :auto-enabled="autoDecisionDiagnosticsEnabled"
	              :running="Boolean(status?.running)"
	              @refresh="refreshDecisionDiagnostics"
	            />
	            <LiveStrategyRuntimeDataPanel
	              v-else
	              :active-data-mode="activeDataMode"
	              :active-panel="focusPanel"
	              :execution-plans="scopedExecutionPlans"
	              :positions="positions"
	              :scoped-equity-history="scopedEquityHistory"
	              :scoped-orders="scopedOrders"
	            />
          </div>
        </section>
        <LiveStrategyRunSummaryPanel :items="strategyKpis" />
      </main>
    </div>
    <LiveStrategyRunParamModal
      v-if="runParamModalOpen"
      :form="form"
      :running="actionLoading"
      :strategy="selectedStrategy"
      @close="runParamModalOpen = false"
      @submit="submitRunParams"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import LiveStrategyControlPanel from '@/components/live/LiveStrategyControlPanel.vue'
import LiveExecutionLogPanel from '@/components/live/LiveExecutionLogPanel.vue'
import LiveStrategyEquityChartPanel from '@/components/live/LiveStrategyEquityChartPanel.vue'
import LiveStrategyRuntimeDataPanel from '@/components/live/LiveStrategyRuntimeDataPanel.vue'
import LiveStrategyRunSummaryPanel from '@/components/live/LiveStrategyRunSummaryPanel.vue'
import LiveStrategyRunParamModal from '@/components/live/LiveStrategyRunParamModal.vue'
	import LiveDecisionDiagnosticsPanel from '@/components/live/LiveDecisionDiagnosticsPanel.vue'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import { useLiveStrategyView } from '@/composables/useLiveStrategyView'
import {
  buildLiveStrategyKpis,
} from '@/utils/liveStrategyDisplay'
import {
  liveEquitySnapshotsForChart,
  liveOrdersForEquityChart,
} from '@/utils/liveStrategyEquityChart'
import {
  buildEquityCandles,
} from '@/utils/strategyExecution'

	type FocusPanel = 'equity-chart' | 'decision' | 'equity-details' | 'positions' | 'planned-exits'

const {
  status,
  positions,
  scopedOrders,
  scopedExecutionPlans,
  scopedEquityHistory,
  executionLogRefreshError,
  executionLogs,
	  currentDecisionDiagnostics,
	  decisionDiagnosticsScopeText,
	  autoDecisionDiagnosticsEnabled,
	  decisionDiagnosticsLoading,
	  decisionDiagnosticsRefreshSource,
	  decisionDiagnosticsError,
  selectedTriggerSymbol,
  error,
  message,
  actionLoading,
  runParamModalOpen,
  form,
  selectedStrategy,
  formLocked,
  strategyOptions,
  controlMode,
  controlModeLabel,
  startDisabledReason,
  stopDisabledReason,
  startButtonText,
  stopButtonText,
  launchReadiness,
  activeDataMode,
  triggerTimeframe,
  triggerTimeframeOptions,
  triggerSymbolOptions,
  detailDataScope,
  runtimeRefreshNotice,
  riskScopeNote,
  setInitialCapital,
  setStrategyId,
  setTriggerSymbol,
  setTriggerTimeframe,
	  loadDecisionDiagnostics,
  openRunParamModal,
  submitRunParams,
  stopStrategy,
} = useLiveStrategyView()

const focusPanel = ref<FocusPanel>('equity-chart')
const liveEquitySnapshots = computed(() => liveEquitySnapshotsForChart(scopedEquityHistory.value))
const liveEquityCandles = computed(() => buildEquityCandles(liveEquitySnapshots.value, triggerTimeframe.value))
const liveEquityTrades = computed(() => liveOrdersForEquityChart(scopedOrders.value))
const strategyKpis = computed(() => buildLiveStrategyKpis({
  status: status.value,
  orders: scopedOrders.value,
  equityHistory: scopedEquityHistory.value,
	  diagnostics: currentDecisionDiagnostics.value,
	  decisionDiagnosticsLoading: decisionDiagnosticsLoading.value,
	  decisionDiagnosticsScopeText: decisionDiagnosticsScopeText.value,
	  autoDecisionDiagnosticsEnabled: autoDecisionDiagnosticsEnabled.value,
	}))

	function refreshDecisionDiagnostics() {
	  void loadDecisionDiagnostics().catch(() => {})
	}

</script>

<style scoped>
.view-live {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-width: 0;
  min-height: 0;
}
.vl-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.vl-title { font-size: 16px; font-weight: 600; margin: 0; }
.vl-status-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
  padding: 4px 10px;
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  color: var(--color-text-secondary);
  font-size: 12px;
}
.vl-status-pill span {
  color: var(--color-text-tertiary);
}
.vl-status-pill strong {
  color: var(--color-text-primary);
  font-weight: 700;
}
.vl-status-pill.running {
  border-color: rgba(38, 166, 154, 0.35);
  background: rgba(38, 166, 154, 0.08);
}
.vl-status-pill.running strong {
  color: var(--color-positive);
}
.vl-message,
.vl-error,
.vl-runtime-warning {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vl-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vl-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vl-runtime-warning {
  border: 1px solid rgba(255,183,77,0.35);
  background: rgba(255,183,77,0.08);
  color: var(--color-warning);
}
.vl-grid {
  flex: 1 1 auto;
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
  min-height: 0;
}
.vl-sidebar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.vl-sidebar :deep(.vl-execution-log-panel) {
  flex: 1 1 220px;
  max-height: none;
}
.vl-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vl-focus-panel {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1 1 58%;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  min-height: 340px;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  overflow: hidden;
}
.vl-tabs {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
  padding: 4px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 6px;
  background: rgba(15, 17, 23, 0.18);
}
.vl-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 7px 12px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  font-weight: 600;
}
.vl-tab:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}
.vl-tab.active {
  border-color: rgba(41, 98, 255, 0.45);
  background: rgba(41, 98, 255, 0.16);
  color: var(--color-text-primary);
}
.vl-tab-count {
  min-width: 20px;
  padding: 1px 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.4;
  text-align: center;
}
.vl-tab.active .vl-tab-count {
  color: var(--color-text-primary);
}
.vl-equity-bucket {
  width: 96px;
  margin-left: auto;
}
.vl-trigger-select {
  width: 180px;
  margin-left: auto;
}
.vl-focus-content {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vl-focus-content.focus-decision,
.vl-focus-content.focus-equity-details,
.vl-focus-content.focus-planned-exits {
  overflow: auto;
  overscroll-behavior: contain;
}
.vl-focus-content :deep(.vl-chart-panel),
.vl-focus-content :deep(.vl-decision-card),
.vl-focus-content :deep(.vl-data-panel) {
  height: 100%;
  min-height: 100%;
}
.vl-focus-content :deep(.vl-decision-card) {
  border: none;
  border-radius: 0;
  background: transparent;
}
.vl-focus-content :deep(.le-panel),
.vl-focus-content :deep(.lf-panel) {
  min-height: 100%;
}
@media (max-width: 1100px) {
  .vl-grid {
    grid-template-columns: 1fr;
  }

  .vl-sidebar {
    max-height: 42vh;
  }

  .vl-topbar {
    align-items: stretch;
    flex-direction: column;
  }

  .vl-main {
    overflow: visible;
  }

  .vl-focus-panel {
    flex: 0 0 auto;
    min-height: 0;
    overflow: visible;
  }

  .vl-focus-content {
    min-height: 320px;
  }

  .vl-trigger-select {
    width: min(100%, 220px);
    margin-left: 0;
  }
}
</style>
