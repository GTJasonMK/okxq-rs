<template>
  <div class="view-settings">
    <h2 class="vs-title">设置</h2>
    <div v-if="message" class="vs-message">{{ message }}</div>
    <div v-if="error" class="vs-error">{{ error }}</div>
    <div v-if="testDetail" class="vs-detail">{{ testDetail }}</div>
    <div class="vs-grid">
      <OkxConfigForm
        :config="okxConfig"
        :saving="savingOkx"
        :testing="testingOkx"
        @save="saveOkxConfig"
        @test="testOkxConfig"
      />
      <SystemStatusPanel :status="systemStore.status" :health="health" @refresh="refreshData" />
      <DataSyncSettingsPanel
        :config="syncRuntimeConfig"
        :saving="savingSyncRuntime"
        @save="saveSyncRuntimeConfig"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettingsView } from '@/composables/useSettingsView'
import OkxConfigForm from '@/components/settings/OkxConfigForm.vue'
import SystemStatusPanel from '@/components/settings/SystemStatusPanel.vue'
import DataSyncSettingsPanel from '@/components/settings/DataSyncSettingsPanel.vue'

const {
  systemStore,
  okxConfig,
  syncRuntimeConfig,
  health,
  error,
  message,
  testDetail,
  savingOkx,
  testingOkx,
  savingSyncRuntime,
  saveOkxConfig,
  saveSyncRuntimeConfig,
  testOkxConfig,
  refreshData,
} = useSettingsView()
</script>

<style scoped>
.view-settings { display: flex; flex-direction: column; gap: 12px; }
.vs-title { font-size: 16px; font-weight: 600; margin: 0; }
.vs-message,
.vs-error,
.vs-detail {
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
.vs-detail {
  border: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
  color: var(--color-text-secondary);
  line-height: 1.5;
}
.vs-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; align-items: start; }
</style>
