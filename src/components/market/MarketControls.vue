<template>
  <div class="vm-controls">
    <div class="vm-market-types">
      <button
        class="vm-toggle"
        :class="{ active: activeMarketType === 'SPOT' }"
        :disabled="!syncSpot"
        @click="$emit('update:active-market-type', 'SPOT')"
      >
        现货
      </button>
      <button
        class="vm-toggle"
        :class="{ active: activeMarketType === 'SWAP' }"
        :disabled="!syncSwap"
        @click="$emit('update:active-market-type', 'SWAP')"
      >
        永续
      </button>
    </div>
    <div class="vm-control-actions">
      <button class="vm-action" :disabled="!hasWatchedSymbol || repairing" @click="$emit('repair-active')">
        {{ repairing ? '补齐中' : '按关注规则补齐' }}
      </button>
      <button class="vm-action" @click="$emit('open-data-center')">
        数据中心
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { MarketInstType } from '@/types/marketView'

defineProps<{
  activeMarketType: MarketInstType
  syncSpot: boolean
  syncSwap: boolean
  hasWatchedSymbol: boolean
  repairing: boolean
}>()

defineEmits<{
  'update:active-market-type': [value: MarketInstType]
  'repair-active': []
  'open-data-center': []
}>()
</script>

<style scoped>
.vm-controls {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
}

.vm-market-types,
.vm-control-actions {
  display: flex;
  gap: 6px;
}

.vm-toggle,
.vm-action {
  min-height: 30px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-secondary);
  color: var(--color-text-secondary);
  cursor: pointer;
}

.vm-toggle.active {
  border-color: var(--color-accent);
  background: var(--color-bg-active);
  color: var(--color-accent);
}

.vm-toggle:disabled,
.vm-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.vm-action:hover:not(:disabled),
.vm-toggle:hover:not(:disabled) {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}
</style>
