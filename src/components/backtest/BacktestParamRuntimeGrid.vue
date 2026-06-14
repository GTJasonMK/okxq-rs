<template>
  <div class="param-grid">
    <div v-for="item in rows" :key="item.label" class="param-item">
      <span>{{ item.label }}</span>
      <strong>{{ item.value }}</strong>
    </div>
  </div>
</template>

<script setup lang="ts">
type RuntimeParamRow = {
  label: string
  value: string
}

defineProps<{
  rows: RuntimeParamRow[]
}>()
</script>

<style scoped>
.param-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 8px;
}
.param-item {
  min-width: 0;
  padding: 7px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
}
.param-item span {
  display: block;
  margin-bottom: 3px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.2;
}
.param-item strong {
  display: block;
  overflow-wrap: anywhere;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 600;
  line-height: 1.25;
}

@media (max-width: 640px) {
  .param-grid {
    grid-template-columns: 1fr;
  }
}
</style>
