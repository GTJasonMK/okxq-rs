<template>
  <div class="param-editor-list">
    <div v-if="rows.length === 0" class="param-empty">{{ emptyText }}</div>
    <div
      v-for="item in rows"
      :key="item.key"
      class="param-readable-row param-editor-row"
      :class="{ group: item.group, multiline: item.multiline }"
      :data-param-key="item.key"
      :style="{ '--depth': item.depth }"
    >
      <div class="param-readable-name">
        <strong>{{ item.label }}</strong>
        <code>{{ item.key }}</code>
      </div>
      <template v-if="!item.group">
        <ThemeSelect
          v-if="item.kind === 'boolean'"
          v-model="item.input"
          class="param-editor-select"
          :options="booleanSelectOptions"
          placeholder="未设置"
          size="md"
        />
        <ThemeSelect
          v-else-if="item.kind === 'select'"
          v-model="item.input"
          class="param-editor-select"
          :options="item.options || []"
          placeholder="请选择"
          size="md"
        />
        <textarea
          v-else-if="item.multiline"
          v-model="item.input"
          class="param-editor-input param-editor-textarea"
          :name="`${namePrefix}-${item.key}`"
          rows="4"
        />
        <input
          v-else
          v-model="item.input"
          class="param-editor-input"
          :name="`${namePrefix}-${item.key}`"
          :type="item.kind === 'number' ? 'number' : 'text'"
          :step="item.kind === 'number' ? 'any' : undefined"
        >
        <span v-if="item.error" class="param-editor-error">{{ item.error }}</span>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ParamDraftRow } from '@/utils/backtestResultCard'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'

defineProps<{
  booleanSelectOptions: Array<{ value: string; label: string }>
  emptyText: string
  namePrefix: string
  rows: ParamDraftRow[]
}>()
</script>

<style scoped>
.param-editor-list {
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
.param-editor-row {
  grid-template-columns: minmax(190px, 0.8fr) minmax(0, 1.2fr);
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
.param-editor-input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.14);
  color: var(--color-text-primary);
  font-size: 12px;
  line-height: 1.35;
  outline: none;
}
.param-editor-select {
  min-width: 0;
}
.param-editor-input:focus {
  border-color: rgba(41, 98, 255, 0.56);
}
.param-editor-textarea {
  min-height: 90px;
  resize: vertical;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  font-size: 11px;
}
.param-editor-error {
  display: block;
  margin-top: 4px;
  color: var(--color-negative);
  font-size: 11px;
  line-height: 1.25;
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
