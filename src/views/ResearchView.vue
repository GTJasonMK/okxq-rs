<template>
  <div class="view-research">
    <h2 class="vr-title">研究平台</h2>
    <div v-if="message" class="vr-message">{{ message }}</div>
    <div v-if="error" class="vr-error">{{ error }}</div>
    <div class="vr-grid">
      <div class="vr-left">
        <DatasetPanel
          :datasets="datasets"
          :active-id="activeDatasetId"
          @select="selectDataset"
          @build="buildDataset"
        />
      </div>
      <div class="vr-right">
        <ModelMetrics :run="activeRun" />
        <TrainingRunList
          :runs="runs"
          :has-dataset="activeDatasetId !== null"
          @train="trainModel"
          @select="selectRun"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useResearchView } from '@/composables/useResearchView'
import DatasetPanel from '@/components/research/DatasetPanel.vue'
import ModelMetrics from '@/components/research/ModelMetrics.vue'
import TrainingRunList from '@/components/research/TrainingRunList.vue'

defineOptions({ name: 'ResearchView' })

const {
  datasets,
  runs,
  activeDatasetId,
  activeRun,
  error,
  message,
  selectDataset,
  buildDataset,
  trainModel,
  selectRun,
} = useResearchView()
</script>

<style scoped>
.view-research { display: flex; flex-direction: column; gap: 12px; }
.vr-title { font-size: 16px; font-weight: 600; margin: 0; }
.vr-message,
.vr-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vr-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vr-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vr-grid { display: grid; grid-template-columns: 300px 1fr; gap: 8px; align-items: start; }
.vr-right { display: flex; flex-direction: column; gap: 8px; }
</style>
