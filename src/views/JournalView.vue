<template>
  <div class="view-journal">
    <div class="vj-header">
      <h2 class="vj-title">交易日志</h2>
      <button class="new-btn" @click="startNew">+ 新建日志</button>
    </div>
    <div v-if="message" class="vj-message">{{ message }}</div>
    <div v-if="error" class="vj-error">{{ error }}</div>
    <div class="vj-grid">
      <JournalList
        :entries="store.entries"
        :active-id="store.activeEntry?.id ?? null"
        @select="selectEntry"
      />
      <JournalEditor
        :entry="store.activeEntry"
        @save="saveEntry"
        @cancel="store.activeEntry = null"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useJournalView } from '@/composables/useJournalView'
import JournalList from '@/components/journal/JournalList.vue'
import JournalEditor from '@/components/journal/JournalEditor.vue'

const {
  store,
  error,
  message,
  saveEntry,
  selectEntry,
  startNew,
} = useJournalView()
</script>

<style scoped>
.view-journal { display: flex; flex-direction: column; height: 100%; gap: 8px; }
.vj-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.vj-title { font-size: 16px; font-weight: 600; margin: 0; }
.vj-message,
.vj-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
}
.vj-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vj-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.new-btn {
  padding: 5px 14px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.vj-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 360px 1fr;
  gap: 8px;
  min-height: 0;
}
</style>
