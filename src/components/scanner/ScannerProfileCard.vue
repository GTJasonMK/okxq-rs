<template>
  <div class="scanner-profiles">
    <div class="sp-header">
      <span class="sp-title">扫描配置</span>
      <button class="icon-btn" @click="toggleCreate" v-if="!showCreate">+ 新建</button>
    </div>

    <ScannerProfileCreateForm
      v-if="showCreate"
      :conditions="conditions"
      @create="handleCreate"
      @cancel="showCreate = false"
    />

    <ScannerProfileList
      v-if="profiles.length > 0"
      :profiles="profiles"
      @run="$emit('run', $event)"
      @delete="$emit('delete', $event)"
    />
    <div v-else class="empty-text">暂无扫描配置</div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import ScannerProfileCreateForm from '@/components/scanner/ScannerProfileCreateForm.vue'
import ScannerProfileList from '@/components/scanner/ScannerProfileList.vue'
import type { ScannerProfile, ScannerCondition } from '@/types'

defineProps<{ profiles: ScannerProfile[]; conditions: ScannerCondition[] }>()
const emit = defineEmits<{
  run: [profile: ScannerProfile]
  delete: [profile: ScannerProfile]
  create: [data: { name: string; conditions: string[] }]
}>()

const showCreate = ref(false)

function toggleCreate() { showCreate.value = !showCreate.value }

function handleCreate(data: { name: string; conditions: string[] }) {
  emit('create', data)
  showCreate.value = false
}
</script>

<style scoped>
.scanner-profiles {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.sp-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sp-title { font-size: 13px; font-weight: 600; }
.icon-btn {
  background: none;
  border: 1px solid var(--color-border);
  padding: 3px 8px;
  border-radius: 3px;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.icon-btn:hover { background: var(--color-bg-hover); }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
