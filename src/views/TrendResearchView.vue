<template>
  <div class="view-trend">
    <h2 class="vt-title">趋势研究</h2>
    <div v-if="message" class="vt-message">{{ message }}</div>
    <div v-if="error" class="vt-error">{{ error }}</div>
    <div class="vt-grid">
      <TrendFactorPanel :factors="factors" :computing="computing" @compute="computeFactors" />
      <TrendConfigForm :config="config" @save="saveConfig" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useTrendResearchView } from '@/composables/useTrendResearchView'
import TrendFactorPanel from '@/components/research/TrendFactorPanel.vue'
import TrendConfigForm from '@/components/research/TrendConfigForm.vue'

defineOptions({ name: 'TrendResearchView' })

const {
  factors,
  computing,
  config,
  error,
  message,
  computeFactors,
  saveConfig,
} = useTrendResearchView()
</script>

<style scoped>
.view-trend { display: flex; flex-direction: column; gap: 12px; }
.vt-title { font-size: 16px; font-weight: 600; margin: 0; }
.vt-message,
.vt-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vt-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vt-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vt-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  align-items: start;
}
</style>
