<template>
  <div class="strategy-selector">
    <div class="ss-header">
      <span class="ss-title">策略</span>
    </div>
    <div class="ss-body">
      <ThemeSelect
        v-model="selectedId"
        :options="strategyOptions"
        placeholder="选择策略..."
      />
      <div v-if="selected" class="ss-info">
        <p class="ss-desc">{{ selected.description }}</p>
        <div v-if="runtimeSummary" class="ss-runtime">{{ runtimeSummary }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { StrategyMeta } from '@/types'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'

const props = defineProps<{
  strategies: StrategyMeta[]
  modelValue?: string
}>()

const emit = defineEmits<{
  'update:strategy-id': [value: string]
}>()

const selectedId = ref(props.modelValue ?? '')

const selected = computed(() =>
  props.strategies.find(s => s.id === selectedId.value) ?? null
)

const strategyOptions = computed(() => [
  { value: '', label: '选择策略...' },
  ...props.strategies.map(strategy => ({ value: strategy.id, label: strategy.name })),
])

const runtimeSummary = computed(() => {
  const runtime = selected.value?.runtime
  if (!runtime) return ''
  const parts = [runtime.symbol, runtime.timeframe, runtime.inst_type].filter(Boolean)
  return parts.length > 0 ? `默认运行：${parts.join(' · ')}` : ''
})

watch(() => props.modelValue, (value) => {
  const nextId = value ?? ''
  if (nextId !== selectedId.value) selectedId.value = nextId
})

watch(selectedId, (id) => {
  emit('update:strategy-id', id)
})
</script>

<style scoped>
.strategy-selector {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
}
.ss-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.ss-title { font-size: 13px; font-weight: 600; }
.ss-body { padding: 10px; display: flex; flex-direction: column; gap: 8px; }
.ss-desc { font-size: 12px; color: var(--color-text-secondary); margin: 0; }
.ss-runtime {
  margin-top: 6px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.4;
}
</style>
