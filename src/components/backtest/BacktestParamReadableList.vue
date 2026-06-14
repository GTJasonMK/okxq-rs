<template>
  <div class="param-readable-list">
    <div v-if="rows.length === 0" class="param-empty">{{ emptyText }}</div>
    <div
      v-for="item in rows"
      :key="item.key"
      class="param-readable-row"
      :class="{ group: item.group, multiline: item.multiline }"
      :style="{ '--depth': item.depth }"
    >
      <div class="param-readable-name">
        <strong>{{ item.label }}</strong>
        <code>{{ item.key }}</code>
      </div>
      <pre v-if="item.multiline" class="param-readable-value multiline">{{ item.value }}</pre>
      <span v-else-if="!item.group" class="param-readable-value">{{ item.value }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ReadableParamRow } from '@/utils/backtestResultCard'

defineProps<{
  emptyText: string
  rows: ReadableParamRow[]
}>()
</script>

<style scoped>
.param-readable-list {
  min-height: 180px;
  max-height: 420px;
  overflow: auto;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
}
.param-readable-row {
  display: grid;
  grid-template-columns: minmax(190px, 0.9fr) minmax(0, 1.1fr);
  gap: 10px;
  align-items: start;
  padding: 8px 10px;
  padding-left: calc(10px + var(--depth, 0) * 16px);
  border-bottom: 1px solid var(--color-border);
}
.param-readable-row:last-child {
  border-bottom: 0;
}
.param-readable-row.group {
  display: block;
  background: rgba(255, 255, 255, 0.025);
}
.param-readable-row.group .param-readable-name {
  margin-bottom: 0;
}
.param-readable-name {
  min-width: 0;
}
.param-readable-name strong {
  display: block;
  overflow-wrap: anywhere;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 600;
  line-height: 1.25;
}
.param-readable-name code {
  display: block;
  margin-top: 3px;
  overflow-wrap: anywhere;
  color: var(--color-text-tertiary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  font-size: 10px;
  line-height: 1.25;
}
.param-readable-value {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  line-height: 1.4;
}
.param-readable-value.multiline {
  max-height: 180px;
  overflow: auto;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.14);
  color: var(--color-text-primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
  font-weight: 500;
  white-space: pre-wrap;
  word-break: break-word;
}
.param-empty {
  padding: 16px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  text-align: center;
}

@media (max-width: 640px) {
  .param-readable-row {
    grid-template-columns: 1fr;
    gap: 5px;
  }
}
</style>
