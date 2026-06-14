<template>
  <div class="rc-header">
    <div class="rc-title-block">
      <span class="rc-title">{{ result.strategy_name || result.strategy_id }}</span>
      <span class="rc-symbol">{{ result.symbol }} · {{ result.timeframe }}</span>
    </div>
    <div class="rc-actions">
      <button
        type="button"
        class="rc-param-btn rc-strategy-param-btn"
        :disabled="running"
        @click="emit('open-strategy-params')"
      >
        策略参数
      </button>
      <button
        type="button"
        class="rc-param-btn rc-engine-param-btn"
        :disabled="running"
        @click="emit('open-engine-params')"
      >
        引擎参数
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { BacktestResult } from '@/types'

defineProps<{
  result: BacktestResult
  running: boolean
}>()

const emit = defineEmits<{
  'open-engine-params': []
  'open-strategy-params': []
}>()
</script>

<style scoped>
.rc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
  padding: 8px 12px 7px;
  border-bottom: 1px solid var(--color-border);
}
.rc-title-block {
  display: flex;
  flex: 1 1 auto;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}
.rc-title {
  min-width: 0;
  overflow: hidden;
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rc-symbol {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.rc-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 6px;
}
.rc-param-btn {
  flex: 0 0 auto;
  padding: 4px 9px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 11px;
  line-height: 1.2;
}
.rc-param-btn:hover {
  border-color: rgba(41, 98, 255, 0.45);
  color: var(--color-text-primary);
}
.rc-param-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

@media (max-width: 640px) {
  .rc-title-block {
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }

  .rc-actions {
    width: 100%;
    justify-content: flex-start;
  }
}
</style>
