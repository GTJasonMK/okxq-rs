<template>
  <div class="dataset-panel">
    <div class="dp-header">
      <span class="dp-title">数据集</span>
      <button class="icon-btn" @click="showBuild = !showBuild">{{ showBuild ? '收起' : '新建' }}</button>
    </div>
    <DatasetBuildForm v-if="showBuild" @build="$emit('build', $event)" />
    <DatasetList
      v-if="datasets.length > 0"
      :datasets="datasets"
      :active-id="activeId"
      @select="$emit('select', $event)"
    />
    <div v-else class="empty-text">暂无数据集</div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import DatasetBuildForm from '@/components/research/DatasetBuildForm.vue'
import DatasetList from '@/components/research/DatasetList.vue'

defineProps<{ datasets: Array<{ id: string; name?: string; created_at?: string }>; activeId: string | null }>()
defineEmits<{ select: [d: Record<string, unknown>]; build: [params: Record<string, unknown>] }>()

const showBuild = ref(false)
</script>

<style scoped>
.dataset-panel {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.dp-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.dp-title { font-size: 13px; font-weight: 600; }
.icon-btn {
  background: none;
  border: 1px solid var(--color-border);
  padding: 2px 8px;
  border-radius: 3px;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.empty-text {
  padding: 24px; text-align: center; color: var(--color-text-tertiary); font-size: 13px;
}
</style>
