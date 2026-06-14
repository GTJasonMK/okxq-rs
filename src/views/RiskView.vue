<template>
  <div class="view-risk">
    <div class="vr-header">
      <h2 class="vr-title">风险监控</h2>
      <button class="refresh-btn" @click="loadData" :disabled="store.loading">
        {{ store.loading ? '刷新中...' : '刷新' }}
      </button>
    </div>
    <div v-if="error" class="vr-error">{{ error }}</div>
    <RiskSummary :metrics="store.varMetrics" :snapshot="store.snapshots[0]" />
    <div class="vr-charts">
      <div class="vr-chart-card">
        <div class="vr-chart-title">回撤曲线</div>
        <DrawdownChart :drawdown="store.drawdown" />
      </div>
      <div class="vr-chart-card">
        <div class="vr-chart-title">滚动指标</div>
        <RollingPanel
          :metrics="(store.rolling as any)?.metrics || []"
          :benchmark="(store.rolling as any)?.benchmark || []"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRiskView } from '@/composables/useRiskView'
import RiskSummary from '@/components/risk/RiskSummary.vue'
import DrawdownChart from '@/components/risk/DrawdownChart.vue'
import RollingPanel from '@/components/risk/RollingPanel.vue'

const { store, error, loadData } = useRiskView()
</script>

<style scoped>
.view-risk { display: flex; flex-direction: column; gap: 12px; }
.vr-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.vr-title { font-size: 16px; font-weight: 600; margin: 0; }
.vr-error {
  padding: 8px 10px;
  border: 1px solid rgba(239,83,80,0.35);
  border-radius: 6px;
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
  font-size: 12px;
}
.refresh-btn {
  padding: 5px 14px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.refresh-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.vr-charts {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.vr-chart-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  padding: 10px;
  padding-bottom: 4px;
}
.vr-chart-title { font-size: 13px; font-weight: 600; margin-bottom: 8px; }
</style>
