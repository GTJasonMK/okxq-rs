<template>
  <div class="dp-list">
    <div
      v-for="dataset in datasets"
      :key="dataset.id"
      class="dp-item"
      :class="{ active: activeId === dataset.id }"
      @click="handleSelect(dataset)"
    >
      <div class="dp-item-name">{{ dataset.name || dataset.id }}</div>
      <div class="dp-item-meta">{{ fmtDate(dataset.created_at || '') }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
type DatasetListItem = {
  id: string
  name?: string
  created_at?: string
}

defineProps<{ datasets: DatasetListItem[]; activeId: string | null }>()
const emit = defineEmits<{ select: [dataset: Record<string, unknown>] }>()

function handleSelect(dataset: DatasetListItem) {
  emit('select', dataset as Record<string, unknown>)
}

function fmtDate(ts: string): string {
  return new Date(ts).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}
</script>

<style scoped>
.dp-list { max-height: 260px; overflow-y: auto; }
.dp-item {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
}
.dp-item:hover { background: var(--color-bg-hover); }
.dp-item.active { background: var(--color-bg-active); border-left: 2px solid var(--color-accent); }
.dp-item-name { font-size: 12px; font-weight: 500; }
.dp-item-meta { font-size: 10px; color: var(--color-text-tertiary); }
</style>
