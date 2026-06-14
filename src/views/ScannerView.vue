<template>
  <div class="view-scanner">
    <div class="vs-header">
      <h2 class="vs-title">扫描</h2>
      <button class="refresh-btn" @click="loadData" :disabled="store.loading">
        {{ store.loading ? '加载中...' : '刷新' }}
      </button>
    </div>
    <div v-if="message" class="vs-message">{{ message }}</div>
    <div v-if="error" class="vs-error">{{ error }}</div>
    <div class="vs-grid">
      <ScannerProfileCard
        :profiles="store.profiles as never"
        :conditions="store.conditions as never"
        @create="createProfile"
        @run="runProfile"
        @delete="deleteProfile"
      />
      <ScannerResults :results="store.results as never" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useScannerView } from '@/composables/useScannerView'
import ScannerProfileCard from '@/components/scanner/ScannerProfileCard.vue'
import ScannerResults from '@/components/scanner/ScannerResults.vue'

const {
  store,
  error,
  message,
  loadData,
  createProfile,
  runProfile,
  deleteProfile,
} = useScannerView()
</script>

<style scoped>
.view-scanner { display: flex; flex-direction: column; height: 100%; gap: 8px; }
.vs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.vs-title { font-size: 16px; font-weight: 600; margin: 0; }
.refresh-btn {
  padding: 5px 14px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.refresh-btn:hover { color: var(--color-text-primary); border-color: var(--color-accent); }
.refresh-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.vs-grid {
  display: grid;
  grid-template-columns: 340px 1fr;
  gap: 8px;
  min-height: 0;
}
.vs-message,
.vs-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vs-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vs-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
</style>
